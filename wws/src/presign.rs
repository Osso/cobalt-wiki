/*
 * presign.rs
 *
 * Wilson's Web Server - Serves a zoo of user-generated content
 * Copyright (C) 2019-2026 Wikijump Team
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */

//! Presigned S3 GET URLs for files on restricted pages.
//!
//! wws redirects a permitted viewer to the object store instead of streaming.
//! The signing time is the start of the UTC day, so one file keeps one URL
//! all day and browsers can cache the object. SigV4 caps expiry at 7 days,
//! so every URL handed out has at least 6 days left.

use crate::attachment::content_disposition;
use crate::deepwell::FileData;
use s3::bucket::Bucket;
use s3::command::Command;
use s3::error::S3Error;
use s3::request::Request;
use s3::request::tokio_backend::ReqwestRequest;
use std::collections::HashMap;
use time::OffsetDateTime;

/// SigV4's maximum presigned URL lifetime.
const EXPIRY_SECS: u32 = 7 * 24 * 60 * 60;
const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

fn start_of_utc_day(now: OffsetDateTime) -> OffsetDateTime {
    let unix = now.unix_timestamp();
    OffsetDateTime::from_unix_timestamp(unix - unix.rem_euclid(SECONDS_PER_DAY))
        .expect("start of a valid day is a valid timestamp")
}

/// rust-s3 appends custom queries in `HashMap` order, which differs between
/// calls. Sorting the already-encoded parameters keeps the URL stable; the
/// signature is unaffected since SigV4 signs the sorted canonical query.
fn sort_query(url: &str) -> String {
    match url.split_once('?') {
        None => url.to_owned(),
        Some((base, query)) => {
            let mut params: Vec<&str> = query.split('&').collect();
            params.sort_unstable();
            format!("{base}?{}", params.join("&"))
        }
    }
}

pub async fn presign_file_get(
    bucket: &Bucket,
    file_info: &FileData,
    filename: &str,
    as_attachment: bool,
    now: OffsetDateTime,
) -> Result<String, S3Error> {
    let disposition = content_disposition(as_attachment, filename);
    let custom_queries = HashMap::from([
        (str!("response-content-type"), file_info.mime.clone()),
        (
            str!("response-content-disposition"),
            String::from_utf8_lossy(disposition.as_bytes()).into_owned(),
        ),
    ]);

    let mut request = ReqwestRequest::new(
        bucket,
        &file_info.s3_hash,
        Command::PresignGet {
            expiry_secs: EXPIRY_SECS,
            custom_queries: Some(custom_queries),
        },
    )
    .await?;
    request.datetime = start_of_utc_day(now);

    let url = request.presigned().await?;
    Ok(sort_query(&url))
}

#[cfg(test)]
mod tests {
    use super::*;
    use s3::creds::Credentials;
    use s3::region::Region;
    use time::{Date, Month, Time};

    /// UTC instant on a September 2026 day.
    fn sept_2026(day: u8, hour: u8, minute: u8, second: u8) -> OffsetDateTime {
        Date::from_calendar_date(2026, Month::September, day)
            .unwrap()
            .with_time(Time::from_hms(hour, minute, second).unwrap())
            .assume_utc()
    }

    fn bucket() -> Box<Bucket> {
        let region = Region::Custom {
            region: str!("auto"),
            endpoint: str!("https://account.r2.cloudflarestorage.com"),
        };
        // AWS documentation example keys.
        let credentials = Credentials::new(
            Some("AKIAIOSFODNN7EXAMPLE"),
            Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"),
            None,
            None,
            None,
        )
        .unwrap();
        Bucket::new("wikijump-files", region, credentials)
            .unwrap()
            .with_path_style()
    }

    fn file() -> FileData {
        FileData {
            file_id: 1,
            mime: str!("font/woff2"),
            size: 1234,
            s3_hash: str!("5f1c0ffee"),
        }
    }

    async fn presign(filename: &str, as_attachment: bool, now: OffsetDateTime) -> String {
        presign_file_get(&bucket(), &file(), filename, as_attachment, now)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn same_day_same_url() {
        let morning =
            presign("liberty-webfont.woff2", false, sept_2026(24, 0, 0, 1)).await;
        let evening =
            presign("liberty-webfont.woff2", false, sept_2026(24, 23, 59, 59)).await;
        assert_eq!(morning, evening);
        assert!(morning.contains("X-Amz-Date=20260924T000000Z"), "{morning}");
        assert!(morning.contains("X-Amz-Expires=604800"), "{morning}");
        assert!(
            morning.starts_with(
                "https://account.r2.cloudflarestorage.com/wikijump-files/5f1c0ffee?"
            ),
            "{morning}",
        );
    }

    #[tokio::test]
    async fn next_day_new_url() {
        let today =
            presign("liberty-webfont.woff2", false, sept_2026(24, 23, 59, 59)).await;
        let tomorrow =
            presign("liberty-webfont.woff2", false, sept_2026(25, 0, 0, 0)).await;
        assert_ne!(today, tomorrow);
        assert!(
            tomorrow.contains("X-Amz-Date=20260925T000000Z"),
            "{tomorrow}"
        );
    }

    #[tokio::test]
    async fn url_overrides_content_type_and_disposition() {
        let now = sept_2026(24, 12, 0, 0);
        let inline = presign("liberty-webfont.woff2", false, now).await;
        assert!(
            inline.contains("response-content-type=font%2Fwoff2"),
            "{inline}"
        );
        assert!(
            inline.contains(
                "response-content-disposition=inline%3B%20filename%3D%22liberty-webfont.woff2%22"
            ),
            "{inline}",
        );

        let download = presign("liberty-webfont.woff2", true, now).await;
        assert!(
            download.contains("response-content-disposition=attachment%3B%20filename%3D"),
            "{download}",
        );
    }

    #[tokio::test]
    async fn reserved_characters_in_filename_stay_encoded() {
        let url = presign("a&b=c+d.png", false, sept_2026(24, 12, 0, 0)).await;
        let query = url.split_once('?').unwrap().1;
        let disposition = query
            .split('&')
            .find(|param| param.starts_with("response-content-disposition="))
            .unwrap();
        assert_eq!(
            disposition,
            "response-content-disposition=inline%3B%20filename%3D%22a%26b%3Dc%2Bd.png%22",
        );
    }

    /// The store accepts the day-rounded signature and applies the overrides.
    /// Needs an `http://` S3 endpoint and wws's S3 variables:
    /// `S3_CUSTOM_ENDPOINT S3_REGION_NAME S3_FILES_BUCKET S3_ACCESS_KEY_ID
    /// S3_SECRET_ACCESS_KEY` (path style).
    #[tokio::test]
    #[ignore = "needs an S3 store, see doc comment"]
    async fn store_serves_presigned_url_with_overrides() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpStream;

        let var = |name| std::env::var(name).unwrap();
        let endpoint = var("S3_CUSTOM_ENDPOINT");
        let region = Region::Custom {
            region: var("S3_REGION_NAME"),
            endpoint: endpoint.clone(),
        };
        let credentials = Credentials::from_env_specific(
            Some("S3_ACCESS_KEY_ID"),
            Some("S3_SECRET_ACCESS_KEY"),
            None,
            None,
        )
        .unwrap();
        let bucket = Bucket::new(&var("S3_FILES_BUCKET"), region, credentials)
            .unwrap()
            .with_path_style();
        let file = FileData {
            file_id: 1,
            mime: str!("image/jpeg"),
            size: 5,
            s3_hash: str!("wws-presign-test-object"),
        };
        bucket.put_object(&file.s3_hash, b"bytes").await.unwrap();

        let url = presign_file_get(
            &bucket,
            &file,
            "chess set&co.jpg",
            false,
            OffsetDateTime::now_utc(),
        )
        .await
        .unwrap();
        let path = &url[endpoint.len()..];
        let host = endpoint.strip_prefix("http://").unwrap();
        let mut stream = TcpStream::connect(host).await.unwrap();
        let request =
            format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).await.unwrap();
        bucket.delete_object(&file.s3_hash).await.unwrap();

        let head = response.to_ascii_lowercase();
        assert!(head.starts_with("http/1.1 200"), "{response}");
        assert!(head.contains("content-type: image/jpeg"), "{response}");
        assert!(
            head.contains("content-disposition: inline; filename=\"chess set&co.jpg\""),
            "{response}",
        );
        assert!(response.ends_with("bytes"), "{response}");
    }
}
