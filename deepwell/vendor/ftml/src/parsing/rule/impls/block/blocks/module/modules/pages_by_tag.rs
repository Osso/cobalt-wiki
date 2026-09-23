/*
 * parsing/rule/impls/block/blocks/module/modules/pages_by_tag.rs
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

pub const MODULE_PAGES_BY_TAG: ModuleRule = ModuleRule {
    name: "module-pages-by-tag",
    accepts_names: &["PagesByTag"],
    parse_fn,
};

fn parse_fn<'r, 't>(
    _parser: &mut Parser<'r, 't>,
    name: &'t str,
    _arguments: Arguments<'t>,
) -> ParseResult<'r, 't, ModuleParseOutput<'t>> {
    debug!("Parsing pages by tag module");
    assert_module_name(&MODULE_PAGES_BY_TAG, name);

    ok!(false; Module::PagesByTag)
}
