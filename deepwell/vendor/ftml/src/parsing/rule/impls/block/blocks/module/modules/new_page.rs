/*
 * parsing/rule/impls/block/blocks/module/modules/new_page.rs
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

pub const MODULE_NEW_PAGE: ModuleRule = ModuleRule {
    name: "module-new-page",
    accepts_names: &["NewPage"],
    parse_fn,
};

fn parse_fn<'r, 't>(
    _parser: &mut Parser<'r, 't>,
    name: &'t str,
    mut arguments: Arguments<'t>,
) -> ParseResult<'r, 't, ModuleParseOutput<'t>> {
    debug!("Parsing new page module");
    assert_module_name(&MODULE_NEW_PAGE, name);

    ok!(false; Module::NewPage {
        category: arguments.get("category"),
        button_text: arguments.get("button"),
        size: arguments.get("size"),
        format: arguments.get("format"),
    })
}
