//! Session-scoped membership applications; no caller-supplied acting user or site.

use super::prelude::*;
use crate::services::member_application::{
    ApplicationEntry, ApplicationStatus, DecideApplication, MemberApplicationService,
    SubmitApplication,
};

pub async fn member_application_get(
    ctx: &ServiceContext<'_>,
    _params: Params<'static>,
) -> Result<ApplicationStatus> {
    MemberApplicationService::get(ctx).await
}

pub async fn member_application_submit(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<()> {
    let input: SubmitApplication = parse!(params, SiteMembership);
    MemberApplicationService::submit(ctx, input).await
}

pub async fn member_application_list(
    ctx: &ServiceContext<'_>,
    _params: Params<'static>,
) -> Result<Vec<ApplicationEntry>> {
    MemberApplicationService::list(ctx).await
}

pub async fn member_application_decide(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<()> {
    let input: DecideApplication = parse!(params, SiteMembership);
    MemberApplicationService::decide(ctx, input).await
}
