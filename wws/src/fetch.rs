/*
 * fetch.rs
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

use crate::deepwell::FileData;
use crate::error::{BasicError, ResponseResult, build_basic_error_response};
use crate::range::ByteRange;
use crate::state::ServerState;
use crate::visibility::PageVisibility;
use axum::body::Body;
use axum::http::header::HeaderMap;
use s3::request::request_trait::ResponseDataStream;
use wikidot_normalize::normalize;

pub async fn fetch_file_info(
    state: &ServerState,
    headers: &HeaderMap,
    site_id: i64,
    page_slug: &mut String,
    filename: &str,
) -> ResponseResult<(FileData, PageVisibility)> {
    normalize(page_slug);

    let page_id = state
        .get_page_or_response(headers, site_id, page_slug)
        .await?;

    // Before the file lookup, so a private page's filenames stay hidden.
    let visibility = state
        .get_page_visibility_or_response(headers, site_id, page_id, page_slug)
        .await?;

    let file_info = state
        .get_file_or_response(headers, site_id, page_id, page_slug, filename)
        .await?;

    Ok((file_info, visibility))
}

pub async fn fetch_full_body(
    state: &ServerState,
    headers: &HeaderMap,
    site_id: i64,
    file_info: &FileData,
    page_slug: &str,
    filename: &str,
) -> ResponseResult<Body> {
    match state
        .s3_files_bucket
        .get_object_stream(&file_info.s3_hash)
        .await
    {
        Ok(ResponseDataStream { bytes, status_code }) => {
            if status_code != 200 {
                error!(
                    site_id = site_id,
                    page_slug = page_slug,
                    filename = filename,
                    s3_hash = &file_info.s3_hash,
                    status_code = status_code,
                    "S3 get_object_stream returned unexpected status",
                );

                let response = build_basic_error_response(
                    state,
                    headers,
                    BasicError::FileFetch {
                        site_id,
                        page_slug,
                        filename,
                    },
                )
                .await;

                return Err(response);
            }

            Ok(Body::from_stream(bytes))
        }
        Err(error) => {
            // NOTE: If the error here is 404 we still return 500.
            //
            //       If we have a file record for a file, then the
            //       corresponding blob *should* exist.
            //
            //       If it doesn't, the data invariant is not being met,
            //       which is an unexpected error.
            error!(
                site_id = site_id,
                page_slug = page_slug,
                filename = filename,
                s3_hash = &file_info.s3_hash,
                "Cannot get blob data: {error}",
            );

            let response = build_basic_error_response(
                state,
                headers,
                BasicError::FileFetch {
                    site_id,
                    page_slug,
                    filename,
                },
            )
            .await;

            Err(response)
        }
    }
}

// Fetch a single byte range as a stream by cloning the bucket and
// injecting an HTTP Range header, so we never buffer the range in memory
pub async fn fetch_range_stream(
    state: &ServerState,
    file_info: &FileData,
    range: ByteRange,
) -> Result<Body, s3::error::S3Error> {
    let mut bucket = (*state.s3_files_bucket).clone();
    bucket.add_header("range", &format!("bytes={}-{}", range.start, range.end));
    let ResponseDataStream { bytes, status_code } =
        bucket.get_object_stream(&file_info.s3_hash).await?;

    if status_code != 206 {
        error!(
            s3_hash = &file_info.s3_hash,
            status_code = status_code,
            "S3 range stream returned unexpected status (expected 206)",
        );
        return Err(s3::error::S3Error::HttpFailWithBody(
            status_code,
            format!("expected 206, got {status_code}"),
        ));
    }

    Ok(Body::from_stream(bytes))
}

// Fetch a single byte range into memory (used for multipart assembly)
pub async fn fetch_range_bytes(
    state: &ServerState,
    file_info: &FileData,
    range: ByteRange,
) -> Result<Vec<u8>, s3::error::S3Error> {
    let resp = state
        .s3_files_bucket
        .get_object_range(&file_info.s3_hash, range.start, Some(range.end))
        .await?;

    if resp.status_code() != 206 {
        error!(
            s3_hash = &file_info.s3_hash,
            status_code = resp.status_code(),
            "S3 range get returned unexpected status (expected 206)",
        );
        return Err(s3::error::S3Error::HttpFailWithBody(
            resp.status_code(),
            format!("expected 206, got {}", resp.status_code()),
        ));
    }

    Ok(resp.to_vec())
}

#[cfg(test)]
mod tests {
    use wikidot_normalize::normalize;

    fn normalized(slug: &str) -> String {
        let mut slug = slug.to_owned();
        normalize(&mut slug);
        slug
    }

    #[test]
    fn page_slug_keeps_later_colons() {
        // Stored Wikidot slug of the page holding chessset.jpg.
        assert_eq!(
            normalized("writing:2021-10-21-to-paint-a-picture:the-game"),
            "writing:2021-10-21-to-paint-a-picture:the-game",
        );
    }

    #[test]
    fn page_slug_still_normalizes_case_and_spaces() {
        assert_eq!(
            normalized("Writing:To Paint A Picture:The Game"),
            "writing:to-paint-a-picture:the-game",
        );
    }
}
