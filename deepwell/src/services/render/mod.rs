/*
 * services/render/mod.rs
 *
 * DEEPWELL - Wikijump API provider and database manager
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

#[allow(unused_imports)]
mod prelude {
    pub use super::super::prelude::*;
    pub use super::structs::*;
    pub use ftml::data::PageInfo;
    pub use ftml::info::VERSION as FTML_VERSION;
    pub use ftml::parsing::ParseError;
    pub use ftml::render::Render;
    pub use ftml::render::html::{HtmlOutput, HtmlRender};
    pub use ftml::settings::WikitextSettings;
    pub use ftml::{self};
}

mod includes;
mod list_pages;
mod listing_deps;
mod live_template;
mod page_tokens;
mod render_data;
mod service;
mod show_to;
mod structs;
mod wikidot_comments;

pub use self::list_pages::ListingSubject;
pub use self::listing_deps::affected_pages as listing_pages_affected_by;
pub use self::service::{BodyArguments, COMPILED_GENERATOR, RenderService};
pub use self::structs::*;
