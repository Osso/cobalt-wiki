use super::prelude::*;
use crate::services::search::{SearchPage, SearchRequest, SearchService};

/// Search the request's trusted site as the request's authenticated actor.
pub async fn page_search(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<SearchPage> {
    let input: SearchRequest = parse!(params, Request);
    SearchService::from_env()?.page_search(ctx, input).await
}
