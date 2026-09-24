//! A fake Mailgun HTTP server for tests that send email.

use deepwell::services::email::MailgunSender;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[allow(unused)] // Only the tests that send email use it.
/// Records the raw requests Mailgun would receive, answering each with 200.
pub async fn fake_mailgun() -> (MailgunSender, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = requests.clone();
    tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0; 4096];
                let size = socket.read(&mut chunk).await.unwrap();
                bytes.extend_from_slice(&chunk[..size]);
                let text = String::from_utf8_lossy(&bytes);
                if let Some((header, body)) = text.split_once("\r\n\r\n") {
                    let length: usize = header
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|value| value.trim().parse().ok())
                        })
                        .unwrap_or(0);
                    if body.len() >= length {
                        break;
                    }
                }
            }
            seen.lock().unwrap().push(String::from_utf8(bytes).unwrap());
            let body = r#"{"message":"Queued. Thank you."}"#;
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len(),
            );
            socket.write_all(reply.as_bytes()).await.unwrap();
        }
    });
    let sender = MailgunSender::new(
        url,
        "mg.example.org".into(),
        "key-test".into(),
        "Cobalt Company <noreply@mg.example.org>".into(),
    );
    (sender, requests)
}

#[allow(unused)] // Only the tests that send email use it.
pub fn form_field(request: &str, name: &str) -> String {
    let body = request.split_once("\r\n\r\n").unwrap().1;
    form_urlencoded::parse(body.as_bytes())
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
        .unwrap_or_default()
}

#[allow(unused)] // Only the tests that send email use it.
pub async fn wait_for_requests(
    requests: &Mutex<Vec<String>>,
    count: usize,
) -> Vec<String> {
    for _ in 0..50 {
        if requests.lock().unwrap().len() >= count {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // Long enough for an unexpected extra email to arrive too.
    tokio::time::sleep(Duration::from_millis(200)).await;
    requests.lock().unwrap().clone()
}
