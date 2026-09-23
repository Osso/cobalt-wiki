//! Explicit integration tests: migrated fixture schema on cobalt_test:25432 only.
use deepwell::models::search_index_pending;
use deepwell::services::search::{SearchService, outbox, worker};
use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DatabaseConnection, EntityTrait, QueryFilter,
    TransactionTrait,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug)]
struct Fixture {
    db: DatabaseConnection,
    admin: DatabaseConnection,
    schema: String,
}

impl Fixture {
    async fn new() -> Self {
        let url = std::env::var("COBALT_TEST_DATABASE_URL")
            .expect("dedicated test database URL required");
        assert!(
            url.contains(":25432/cobalt_test"),
            "only cobalt_test:25432 is permitted"
        );
        let admin = Database::connect(&url).await.unwrap();
        let schema = format!("search_test_{}", uuid::Uuid::new_v4().simple());
        admin
            .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
            .await
            .unwrap();
        let separator = if url.contains('?') { '&' } else { '?' };
        let scoped_url =
            format!("{url}{separator}options=-csearch_path%3D{schema}%2Cpublic");
        let db = Database::connect(scoped_url).await.unwrap();
        db.execute_unprepared(include_str!(
            "../migrations/20260923000001_search_index_pending.sql"
        ))
        .await
        .unwrap();
        db.execute_unprepared("DELETE FROM search_index_pending")
            .await
            .unwrap();
        Self { db, admin, schema }
    }

    async fn close(self) {
        self.db.close().await.unwrap();
        self.admin
            .execute_unprepared(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .await
            .unwrap();
    }

    async fn existing_page(&self) -> i64 {
        use deepwell::models::page;
        page::Entity::find()
            .filter(page::Column::DeletedAt.is_null())
            .one(&self.db)
            .await
            .unwrap()
            .expect("test database needs a live page fixture")
            .page_id
    }
}

#[tokio::test]
#[ignore = "requires dedicated cobalt_test:25432 database"]
async fn rollback_and_generation_ack_preserve_newer_work() {
    let fixture = Fixture::new().await;
    let db = &fixture.db;
    let page_id = fixture.existing_page().await;
    let txn = db.begin().await.unwrap();
    outbox::enqueue(&txn, page_id).await.unwrap();
    assert!(
        search_index_pending::Entity::find_by_id(page_id)
            .one(&txn)
            .await
            .unwrap()
            .is_some()
    );
    txn.rollback().await.unwrap();
    assert!(
        search_index_pending::Entity::find_by_id(page_id)
            .one(db)
            .await
            .unwrap()
            .is_none()
    );

    let txn = db.begin().await.unwrap();
    outbox::enqueue(&txn, page_id).await.unwrap();
    let first = search_index_pending::Entity::find_by_id(page_id)
        .one(&txn)
        .await
        .unwrap()
        .unwrap();
    txn.commit().await.unwrap();
    let txn = db.begin().await.unwrap();
    outbox::enqueue(&txn, page_id).await.unwrap();
    txn.commit().await.unwrap();
    let txn = db.begin().await.unwrap();
    assert!(
        !outbox::acknowledge(&txn, page_id, first.generation)
            .await
            .unwrap()
    );
    let second = search_index_pending::Entity::find_by_id(page_id)
        .one(&txn)
        .await
        .unwrap()
        .unwrap();
    assert!(second.generation > first.generation);
    assert!(
        outbox::acknowledge(&txn, page_id, second.generation)
            .await
            .unwrap()
    );
    txn.commit().await.unwrap();
    fixture.close().await;
}

#[tokio::test]
#[ignore = "requires dedicated cobalt_test:25432 database"]
async fn failed_external_write_retains_pending_row() {
    let fixture = Fixture::new().await;
    let db = &fixture.db;
    let page_id = fixture.existing_page().await;
    let txn = db.begin().await.unwrap();
    outbox::enqueue(&txn, page_id).await.unwrap();
    txn.commit().await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let responder = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 8192];
        socket.read(&mut request).await.unwrap();
        socket.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
    });
    let search = SearchService::new(url, "test-key".into());
    assert!(worker::process_one_batch(db, &search).await.is_err());
    responder.await.unwrap();
    assert!(
        search_index_pending::Entity::find_by_id(page_id)
            .one(db)
            .await
            .unwrap()
            .is_some()
    );
    fixture.close().await;
}

#[tokio::test]
#[ignore = "requires dedicated cobalt_test:25432 database"]
async fn concurrent_commit_keeps_new_generation_and_workers_serialize() {
    use tokio::sync::oneshot;
    use tokio::time::{Duration, timeout};
    let fixture = Fixture::new().await;
    let db = &fixture.db;
    let page_id = fixture.existing_page().await;
    let txn = db.begin().await.unwrap();
    outbox::enqueue(&txn, page_id).await.unwrap();
    txn.commit().await.unwrap();
    let old_generation = search_index_pending::Entity::find_by_id(page_id)
        .one(db)
        .await
        .unwrap()
        .unwrap()
        .generation;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (request_seen, request_ready) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let responder = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = [0; 8192];
        socket.read(&mut bytes).await.unwrap();
        request_seen.send(()).unwrap();
        released.await.unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 14\r\nConnection: close\r\n\r\n{\"taskUid\":21}").await.unwrap();
        let (mut socket, _) = listener.accept().await.unwrap();
        socket.read(&mut bytes).await.unwrap();
        socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 22\r\nConnection: close\r\n\r\n{\"status\":\"succeeded\"}").await.unwrap();
    });
    let service = SearchService::new(url, "test-key".into());
    let worker_db = db.clone();
    let worker_service = service.clone();
    let first = tokio::spawn(async move {
        worker::process_one_batch(&worker_db, &worker_service).await
    });
    timeout(Duration::from_secs(5), request_ready)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(worker::process_one_batch(db, &service).await.unwrap(), 0);
    let txn = db.begin().await.unwrap();
    outbox::enqueue(&txn, page_id).await.unwrap();
    timeout(Duration::from_secs(5), txn.commit())
        .await
        .unwrap()
        .unwrap();
    release.send(()).unwrap();
    assert_eq!(
        timeout(Duration::from_secs(5), first)
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
        1
    );
    responder.await.unwrap();
    let pending = search_index_pending::Entity::find_by_id(page_id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert!(pending.generation > old_generation);
    fixture.close().await;
}
