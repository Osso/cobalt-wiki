//! Member administration for site admins. The acting user and site come
//! from the request context (session token and site headers), never from
//! the parameters; see `MemberAdminService`.

use super::prelude::*;
use crate::services::MemberAdminService;
use crate::services::member_admin::{
    InviteMember, InviteMemberOutput, RemoveMember, SetMemberRole, SiteMemberEntry,
};

pub async fn member_admin_list(
    ctx: &ServiceContext<'_>,
    _params: Params<'static>,
) -> Result<Vec<SiteMemberEntry>> {
    MemberAdminService::list(ctx)
        .await
        .or_raise(|| Error::new("failed to list site members", ErrorType::SiteMembership))
}

pub async fn member_admin_set_role(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<()> {
    let input: SetMemberRole = parse!(params, SiteMembership);
    MemberAdminService::set_role(ctx, input).await.or_raise(|| {
        Error::new("failed to change member role", ErrorType::SiteMembership)
    })
}

pub async fn member_admin_remove(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<()> {
    let input: RemoveMember = parse!(params, SiteMembership);
    MemberAdminService::remove(ctx, input)
        .await
        .or_raise(|| Error::new("failed to remove member", ErrorType::SiteMembership))
}

pub async fn member_admin_invite(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<InviteMemberOutput> {
    let input: InviteMember = parse!(params, SiteMembership);
    MemberAdminService::invite(ctx, input)
        .await
        .or_raise(|| Error::new("failed to invite member", ErrorType::SiteMembership))
}
