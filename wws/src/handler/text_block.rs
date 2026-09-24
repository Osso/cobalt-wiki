/*
 * handler/text_block.rs
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
use crate::deepwell::{TextBlockIndex, TextBlockType};
use crate::error::{
    BasicError, FallbackError, TextBlockErrorReason, build_basic_error_response,
};
use crate::state::ServerState;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::{self, HeaderMap};
use axum::http::status::StatusCode;
use axum::response::{IntoResponse, Response};
use std::collections::HashMap;
use std::num::NonZeroU16;

pub async fn handle_html_block(
    State(state): State<ServerState>,
    Path((page_slug, index)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    info!(
        page_slug = page_slug,
        index = index,
        "Returning HTML block data",
    );

    // HTML blocks can't have named aliases
    handle_text_block(
        &state,
        &headers,
        TextBlockType::Html,
        &page_slug,
        BlockId::Index(index),
    )
    .await
}

pub async fn handle_code_block(
    State(state): State<ServerState>,
    Path((page_slug, value)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    info!(
        page_slug = page_slug,
        index = value,
        "Returning code block data",
    );

    // Determine if it's an alias or a regular numeric index
    let index = if value.chars().all(|c| c.is_ascii_digit()) {
        BlockId::Index(value)
    } else {
        BlockId::Name(value)
    };

    handle_text_block(&state, &headers, TextBlockType::Code, &page_slug, index).await
}

async fn handle_text_block(
    state: &ServerState,
    headers: &HeaderMap,
    block_type: TextBlockType,
    page_slug: &str,
    block_id: BlockId,
) -> Response {
    let site_id = get_site_id(headers);
    let page_id = try_response!(state.get_page_or_response(headers, site_id, page_slug));
    let _visibility = try_response!(
        state.get_page_visibility_or_response(headers, site_id, page_id, page_slug)
    );

    let (index, s3_filename) = match block_id {
        // Parse the index value if numeric
        BlockId::Index(value) => match value.parse() {
            Ok(index) => {
                let s3_filename = format_filename(block_type, page_id, index);
                (index, s3_filename)
            }
            Err(_) => {
                error!(
                    index = value,
                    block_type = block_type.value(),
                    "Invalid text block index",
                );
                return build_basic_error_response(
                    state,
                    headers,
                    BasicError::TextBlock {
                        site_id,
                        index: &value,
                        block_type,
                        reason: TextBlockErrorReason::Invalid,
                    },
                )
                .await;
            }
        },
        // Retrieve the index from DEEPWELL
        BlockId::Name(name) => {
            match state
                .deepwell
                .get_text_block_index(page_id, block_type, &name)
                .await
            {
                Ok(Some(TextBlockIndex { index, s3_filename })) => (index, s3_filename),
                Ok(None) => {
                    error!(
                        page_id = page_id,
                        block_type = block_type.value(),
                        name = name,
                        "No text block found with given name",
                    );
                    return build_basic_error_response(
                        state,
                        headers,
                        BasicError::TextBlock {
                            site_id,
                            index: &name,
                            block_type,
                            reason: TextBlockErrorReason::Missing,
                        },
                    )
                    .await;
                }
                Err(error) => {
                    error!(
                        page_id = page_id,
                        block_type = block_type.value(),
                        "Unable to retrieve S3 filename for text block from DEEPWELL: {error}",
                    );
                    return build_basic_error_response(
                        state,
                        headers,
                        BasicError::TextBlock {
                            site_id,
                            index: &name,
                            block_type,
                            reason: TextBlockErrorReason::Fetch,
                        },
                    )
                    .await;
                }
            }
        }
    };

    info!("Fetching HTML text block from S3 object '{s3_filename}' (index {index})");

    // Since text blocks are much smaller than files (necessarily being
    // at most as big as the biggest page's sources) then it's fine for
    // us to download the whole thing to memory instead of streaming it.
    let s3_response = match state.s3_tblocks_bucket.get_object(&s3_filename).await {
        Ok(response) => {
            assert_eq!(
                response.status_code(),
                StatusCode::OK,
                "get_object() succeeded but did not reply 200",
            );

            response
        }
        Err(error) => {
            // NOTE: If the error here is 404 we still return 500.
            //
            //       If we have a file record for a file, then the
            //       corresponding blob *should* exist.
            //
            //       If it doesn't, the data invariant is not being met,
            //       which is an unexpected error.
            //
            //       Fallback error code: XF-1004
            error!(
                page_id = page_id,
                block_type = "html",
                s3_filename = s3_filename,
                "Cannot get text block data: {error}",
            );
            return FallbackError::TextBlockS3Fetch.into_response();
        }
    };

    let Headers { content_type, etag } = get_headers(s3_response.headers());
    let body = Body::from({
        // Ensure text blocks always end in a newline.
        // This doesn't make the additional conditional to
        // avoid confusing behavior.
        let mut bytes = s3_response.to_vec();
        bytes.push(b'\n');
        bytes
    });
    let result = Response::builder()
        .header(header::CONTENT_TYPE, &content_type)
        .header(header::ETAG, &etag)
        .body(body);

    match result {
        Ok(response) => response,
        Err(error) => {
            error!("Unable to convert response: {error}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Debug)]
enum BlockId {
    Index(String),
    Name(String),
}

#[derive(Debug)]
struct Headers {
    content_type: String,
    etag: String,
}

/// Formats the S3 filename for a hosted text block.
/// See `service/text_block/service.rs` for how this value is formatted.
#[inline]
fn format_filename(block_type: TextBlockType, page_id: i64, index: NonZeroU16) -> String {
    let block_type = block_type.value();
    format!("{page_id}_{block_type}_{index}")
}

// Since this thing isn't returning a case-insensitive map...
fn get_headers(headers: HashMap<String, String>) -> Headers {
    let mut content_type = None;
    let mut etag = None;

    for (key, value) in headers.into_iter() {
        if key.eq_ignore_ascii_case("content-type") {
            content_type = Some(value);
        } else if key.eq_ignore_ascii_case("etag") {
            etag = Some(value);
        }
    }

    Headers {
        content_type: content_type.expect("No Content-Type header in S3 response"),
        etag: etag.expect("No ETag header in S3 response"),
    }
}
