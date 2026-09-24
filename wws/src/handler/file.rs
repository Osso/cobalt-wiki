/*
 * handler/file.rs
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

use super::get_site_id;
use crate::attachment::content_disposition;
use crate::deepwell::FileData;
use crate::error::{BasicError, build_basic_error_response};
use crate::fetch::{
    fetch_file_info, fetch_full_body, fetch_range_bytes, fetch_range_stream,
};
use crate::presign::presign_file_get;
use crate::range::{ByteRange, ParsedRange, evaluate_range};
use crate::state::ServerState;
use crate::visibility::{CACHE_PRIVATE, CACHE_PUBLIC, PageVisibility};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::{self, HeaderMap};
use axum::http::{Method, StatusCode};
use axum::response::{IntoResponse, Response};
use rand::distr::{Alphanumeric, SampleString};
use std::fmt::Write;
use time::OffsetDateTime;

/// Prefix for MIME boundaries used in `multipart/byteranges` responses.
///
/// See RFC 2046 section 5.1.1:
/// https://www.rfc-editor.org/rfc/rfc2046.html#section-5.1.1
const MULTIPART_BOUNDARY_PREFIX: &str = "wikijump_byteranges_";
const MULTIPART_BOUNDARY_RANDOM_LENGTH: usize = 16;

/// Maximum total bytes we'll buffer for a `multipart/byteranges` response.
/// Beyond this, the multipart request is rejected with 416 (Range Not Satisfiable)
const MAX_MULTIPART_BYTES: u64 = 8 * 1024 * 1024; // 8 MiB

fn range_not_satisfiable(file_size: u64) -> Response {
    build_or_500(
        Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(header::CONTENT_RANGE, format!("bytes */{file_size}"))
            .header(header::ACCEPT_RANGES, "bytes")
            .body(Body::empty()),
    )
}

struct ServeParams<'a> {
    etag: &'a str,
    as_attachment: bool,
    filename: &'a str,
    file_size: u64,
    is_head: bool,
}

async fn serve_file(
    state: &ServerState,
    method: &Method,
    headers: &HeaderMap,
    file_info: &FileData,
    as_attachment: bool,
    page_slug: &str,
    filename: &str,
) -> Response {
    let file_size = file_info.size as u64;
    let etag = format!("\"{}\"", file_info.s3_hash);
    let is_head = *method == Method::HEAD;
    let params = ServeParams {
        etag: &etag,
        as_attachment,
        filename,
        file_size,
        is_head,
    };

    match evaluate_range(headers, &etag, file_size) {
        ParsedRange::None => {
            serve_full(state, headers, file_info, page_slug, &params).await
        }
        ParsedRange::NotSatisfiable => range_not_satisfiable(file_size),
        ParsedRange::Satisfiable(ref ranges) if ranges.len() == 1 => {
            serve_single_range(state, file_info, ranges[0], &params).await
        }
        ParsedRange::Satisfiable(ranges) => {
            let total: u64 = ranges.iter().map(|r| r.len()).sum();
            if total > MAX_MULTIPART_BYTES {
                return range_not_satisfiable(file_size);
            }

            serve_multi_range(state, file_info, &ranges, &params).await
        }
    }
}

async fn serve_full(
    state: &ServerState,
    headers: &HeaderMap,
    file_info: &FileData,
    page_slug: &str,
    params: &ServeParams<'_>,
) -> Response {
    let body = if params.is_head {
        Body::empty()
    } else {
        match fetch_full_body(
            state,
            headers,
            get_site_id(headers),
            file_info,
            page_slug,
            params.filename,
        )
        .await
        {
            Ok(b) => b,
            Err(resp) => return resp,
        }
    };

    build_or_500(
        base_headers(
            StatusCode::OK,
            params.etag,
            params.as_attachment,
            params.filename,
        )
        .header(header::CONTENT_TYPE, &file_info.mime)
        .header(header::CONTENT_LENGTH, params.file_size)
        .body(body),
    )
}

async fn serve_single_range(
    state: &ServerState,
    file_info: &FileData,
    range: ByteRange,
    params: &ServeParams<'_>,
) -> Response {
    let body = if params.is_head {
        Body::empty()
    } else {
        match fetch_range_stream(state, file_info, range).await {
            Ok(b) => b,
            Err(error) => {
                error!(
                    s3_hash = &file_info.s3_hash,
                    start = range.start,
                    end = range.end,
                    "S3 range fetch failed: {error}",
                );
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    };

    let content_range =
        format!("bytes {}-{}/{}", range.start, range.end, params.file_size);

    build_or_500(
        base_headers(
            StatusCode::PARTIAL_CONTENT,
            params.etag,
            params.as_attachment,
            params.filename,
        )
        .header(header::CONTENT_TYPE, &file_info.mime)
        .header(header::CONTENT_RANGE, content_range)
        .header(header::CONTENT_LENGTH, range.len())
        .body(body),
    )
}

async fn serve_multi_range(
    state: &ServerState,
    file_info: &FileData,
    ranges: &[ByteRange],
    params: &ServeParams<'_>,
) -> Response {
    let boundary = generate_multipart_boundary();
    let content_type = format!("multipart/byteranges; boundary={boundary}");

    if params.is_head {
        let len = multipart_content_length(
            &boundary,
            &file_info.mime,
            ranges,
            params.file_size,
        );
        return build_or_500(
            base_headers(
                StatusCode::PARTIAL_CONTENT,
                params.etag,
                params.as_attachment,
                params.filename,
            )
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_LENGTH, len)
            .body(Body::empty()),
        );
    }

    let mut body = Vec::new();

    for range in ranges {
        let data = match fetch_range_bytes(state, file_info, *range).await {
            Ok(d) => d,
            Err(error) => {
                error!(
                    s3_hash = &file_info.s3_hash,
                    start = range.start,
                    end = range.end,
                    "S3 range fetch failed: {error}",
                );
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        };

        let mut part_header = String::new();
        let _ = write!(
            part_header,
            "--{boundary}\r\n\
             Content-Type: {}\r\n\
             Content-Range: bytes {}-{}/{}\r\n\
             \r\n",
            file_info.mime, range.start, range.end, params.file_size,
        );
        body.extend_from_slice(part_header.as_bytes());
        body.extend_from_slice(&data);
        body.extend_from_slice(b"\r\n");
    }

    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    build_or_500(
        base_headers(
            StatusCode::PARTIAL_CONTENT,
            params.etag,
            params.as_attachment,
            params.filename,
        )
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, body.len())
        .body(Body::from(body)),
    )
}

// ------------ Public handlers ------------

pub async fn handle_file_fetch(
    State(state): State<ServerState>,
    method: Method,
    Path((page_slug, filename)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    info!(
        page_slug = page_slug,
        filename = filename,
        "Returning file data",
    );

    handle_file(&state, &method, &headers, page_slug, &filename, false).await
}

pub async fn handle_file_download(
    State(state): State<ServerState>,
    method: Method,
    Path((page_slug, filename)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    info!(
        page_slug = page_slug,
        filename = filename,
        "Returning file download",
    );

    handle_file(&state, &method, &headers, page_slug, &filename, true).await
}

/// Streams files of anonymously viewable pages (publicly cacheable);
/// redirects a permitted viewer of a restricted page to a presigned URL.
async fn handle_file(
    state: &ServerState,
    method: &Method,
    headers: &HeaderMap,
    mut page_slug: String,
    filename: &str,
    as_attachment: bool,
) -> Response {
    let site_id = get_site_id(headers);
    let (file_info, visibility) =
        match fetch_file_info(state, headers, site_id, &mut page_slug, filename).await {
            Ok(info) => info,
            Err(response) => return response,
        };

    match visibility {
        PageVisibility::Public => {
            serve_file(
                state,
                method,
                headers,
                &file_info,
                as_attachment,
                &page_slug,
                filename,
            )
            .await
        }
        PageVisibility::Restricted => {
            redirect_presigned(
                state,
                headers,
                &file_info,
                &page_slug,
                filename,
                as_attachment,
            )
            .await
        }
    }
}

async fn redirect_presigned(
    state: &ServerState,
    headers: &HeaderMap,
    file_info: &FileData,
    page_slug: &str,
    filename: &str,
    as_attachment: bool,
) -> Response {
    let now = OffsetDateTime::now_utc();
    match presign_file_get(
        &state.s3_files_bucket,
        file_info,
        filename,
        as_attachment,
        now,
    )
    .await
    {
        Ok(url) => build_or_500(
            Response::builder()
                .status(StatusCode::FOUND)
                .header(header::LOCATION, url)
                .header(header::CACHE_CONTROL, CACHE_PRIVATE)
                .body(Body::empty()),
        ),
        Err(error) => {
            error!(
                s3_hash = &file_info.s3_hash,
                "Cannot presign file URL: {error}",
            );
            build_basic_error_response(
                state,
                headers,
                BasicError::FileFetch {
                    site_id: get_site_id(headers),
                    page_slug,
                    filename,
                },
            )
            .await
        }
    }
}

// ------------ Response builders ------------

/// Only files of anonymously viewable pages are streamed.
fn base_headers(
    status: StatusCode,
    etag: &str,
    as_attachment: bool,
    filename: &str,
) -> axum::http::response::Builder {
    let mut builder = Response::builder()
        .status(status)
        .header(header::ETAG, etag)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CACHE_CONTROL, CACHE_PUBLIC);

    if as_attachment {
        builder = builder.header(
            header::CONTENT_DISPOSITION,
            content_disposition(true, filename),
        );
    }

    builder
}

fn build_or_500(result: Result<Response<Body>, axum::http::Error>) -> Response {
    match result {
        Ok(r) => r,
        Err(error) => {
            error!("Unable to build response: {error}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn generate_multipart_boundary() -> String {
    let mut rng = rand::rng();
    let mut boundary = String::with_capacity(
        MULTIPART_BOUNDARY_PREFIX.len() + MULTIPART_BOUNDARY_RANDOM_LENGTH,
    );

    boundary.push_str(MULTIPART_BOUNDARY_PREFIX);
    Alphanumeric.append_string(&mut rng, &mut boundary, MULTIPART_BOUNDARY_RANDOM_LENGTH);

    boundary
}

// Compute the `Content-Length` of a `multipart/byteranges` body (so HEAD can skip s3)
fn multipart_content_length(
    boundary: &str,
    mime: &str,
    ranges: &[ByteRange],
    file_size: u64,
) -> usize {
    let mut len: usize = 0;
    for range in ranges {
        // --boundary\r\n
        len += 2 + boundary.len() + 2;
        // Content-Type: {mime}\r\n
        len += "Content-Type: ".len() + mime.len() + 2;
        // Content-Range: bytes {start}-{end}/{file_size}\r\n
        let cr = format!(
            "Content-Range: bytes {}-{}/{file_size}\r\n",
            range.start, range.end
        );
        len += cr.len();
        // blank line
        len += 2;
        // data
        len += range.len() as usize;
        // trailing \r\n
        len += 2;
    }
    // --boundary--\r\n
    len += 2 + boundary.len() + 2 + 2;
    len
}
