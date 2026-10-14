//! Join-module display is personalized only after shared rendering is complete.

use super::GetPageViewOutput;
use crate::error::Result;
use crate::services::ServiceContext;
use crate::services::relation::{GetSiteMember, RelationService};
use scraper::{Html, Selector};

pub(super) async fn hide_member_join(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    user_id: Option<i64>,
    mut output: GetPageViewOutput,
) -> Result<GetPageViewOutput> {
    let Some(user_id) = user_id else {
        return Ok(output);
    };
    let membership = RelationService::get_optional_site_member(
        ctx,
        GetSiteMember { site_id, user_id },
    )
    .await?;
    if membership.is_none() {
        return Ok(output);
    }
    remove_join_from_output(&mut output);
    Ok(output)
}

fn remove_join_from_output(output: &mut GetPageViewOutput) {
    let (body, top, side) = match output {
        GetPageViewOutput::Found {
            compiled_body_html,
            compiled_top_bar_html,
            compiled_side_bar_html,
            ..
        }
        | GetPageViewOutput::Missing {
            compiled_body_html,
            compiled_top_bar_html,
            compiled_side_bar_html,
            ..
        }
        | GetPageViewOutput::Permissions {
            compiled_body_html,
            compiled_top_bar_html,
            compiled_side_bar_html,
            ..
        } => (
            compiled_body_html,
            compiled_top_bar_html,
            compiled_side_bar_html,
        ),
    };
    for html in std::iter::once(body).chain(top).chain(side) {
        *html = remove_join_modules(html);
    }
}

fn remove_join_modules(html: &str) -> String {
    if !html.contains("join-box") {
        return html.to_owned();
    }
    let mut dom = Html::parse_fragment(html);
    // FTML owns this wrapper; ordinary links to /-/join are not Join modules.
    let selector = Selector::parse("div.join-box").expect("static Join selector");
    let modules: Vec<_> = dom.select(&selector).map(|node| node.id()).collect();
    for id in modules {
        dom.tree.get_mut(id).expect("selected Join node").detach();
    }
    dom.root_element().inner_html()
}
