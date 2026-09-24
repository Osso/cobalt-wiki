/*
 * render/html/element/button.rs
 *
 * ftml - Library to parse Wikidot text
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

use super::prelude::*;

/// Wikidot's `<a href="javascript:;" class="wiki-standalone-button">`,
/// without its `onclick` handler. The href keeps it focusable and
/// keyboard-activatable; the frontend wires the action from
/// `data-button-type` (and `data-tags` for `set-tags`).
pub fn render_button(ctx: &mut HtmlContext, button_type: &str, text: &str, tags: &str) {
    debug!("Rendering button (type '{button_type}', tags '{tags}')");

    ctx.html()
        .a()
        .attr(attr!(
            "href" => "javascript:;",
            "class" => "wiki-standalone-button",
            "data-button-type" => button_type,
            "data-tags" => tags; if !tags.is_empty(),
        ))
        .contents(text);
}
