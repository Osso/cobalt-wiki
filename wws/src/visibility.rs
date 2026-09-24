/*
 * visibility.rs
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

//! Page/View gate for content served by page (files and text blocks).
//!
//! The viewer is the session in Framerail's login cookie, which reaches wws
//! on the same origin. DEEPWELL runs the same check as the page view.

use crate::deepwell::PageViewPermission;
use axum::http::StatusCode;
use axum::http::header::{self, HeaderMap};
use axum::response::{IntoResponse, Response};
use headers::{Cookie, HeaderMapExt};

/// Framerail's session cookie.
const SESSION_COOKIE: &str = "wikijump_token";

const CACHE_PRIVATE: &str = "private, no-store";

pub fn get_session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .typed_get::<Cookie>()?
        .get(SESSION_COOKIE)
        .map(str::to_owned)
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PageVisibility {
    /// Anonymous visitors may view the page, so shared caches may keep its content.
    Public,
    /// Only some viewers may view the page.
    Restricted,
}

impl PageVisibility {
    /// `None` when the viewer may not view the page.
    pub fn for_viewer(
        PageViewPermission { can_view, public }: PageViewPermission,
    ) -> Option<Self> {
        match (can_view, public) {
            (false, _) => None,
            (true, true) => Some(PageVisibility::Public),
            (true, false) => Some(PageVisibility::Restricted),
        }
    }
}

/// Same status as the page view's permissions page.
pub fn forbidden_response() -> Response {
    (
        StatusCode::FORBIDDEN,
        [(header::CACHE_CONTROL, CACHE_PRIVATE)],
        "Forbidden",
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn cookie_headers(value: &'static str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static(value));
        headers
    }

    #[test]
    fn session_token_from_login_cookie() {
        let headers = cookie_headers("theme=dark; wikijump_token=wj:abc123; lang=en");
        assert_eq!(get_session_token(&headers).as_deref(), Some("wj:abc123"));
    }

    #[test]
    fn no_session_token_without_login_cookie() {
        assert_eq!(get_session_token(&HeaderMap::new()), None);
        assert_eq!(get_session_token(&cookie_headers("theme=dark")), None);
    }

    #[test]
    fn denied_viewer_gets_no_content() {
        for public in [false, true] {
            let permission = PageViewPermission {
                can_view: false,
                public,
            };
            assert_eq!(PageVisibility::for_viewer(permission), None);
        }
    }

    #[test]
    fn permitted_viewer_gets_page_visibility() {
        let visibility = |public| {
            PageVisibility::for_viewer(PageViewPermission {
                can_view: true,
                public,
            })
        };
        assert_eq!(visibility(true), Some(PageVisibility::Public));
        assert_eq!(visibility(false), Some(PageVisibility::Restricted));
    }

    #[test]
    fn forbidden_response_is_403_and_not_stored() {
        let response = forbidden_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "private, no-store"
        );
    }
}
