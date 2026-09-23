/*
 * parsing/rule/impls/block/blocks/module/modules/tag_cloud.rs
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

pub const MODULE_TAG_CLOUD: ModuleRule = ModuleRule {
    name: "module-tag-cloud",
    accepts_names: &["TagCloud"],
    parse_fn,
};

fn parse_fn<'r, 't>(
    _parser: &mut Parser<'r, 't>,
    name: &'t str,
    mut arguments: Arguments<'t>,
) -> ParseResult<'r, 't, ModuleParseOutput<'t>> {
    debug!("Parsing tag cloud module");
    assert_module_name(&MODULE_TAG_CLOUD, name);

    ok!(false; Module::TagCloud {
        limit: arguments.get("limit"),
        target: arguments.get("target"),
        min_font_size: arguments.get("minFontSize"),
        max_font_size: arguments.get("maxFontSize"),
        min_color: arguments.get("minColor"),
        max_color: arguments.get("maxColor"),
    })
}
