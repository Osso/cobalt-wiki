use super::prelude::*;
use crate::services::PasswordTokenService;
use crate::services::password_token::{
    CreatePasswordLink, CreatePasswordLinkOutput, PasswordLinkStatus,
    RedeemPasswordToken, RequestPasswordReset,
};
use crate::services::user::GetUser;

/// Trusted callers only (the member invite tool): returns a working link.
pub async fn password_token_create(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<CreatePasswordLinkOutput> {
    let input: CreatePasswordLink = parse!(params, User);
    PasswordTokenService::create_link(ctx, input)
        .await
        .or_raise(|| Error::new("failed to create password link", ErrorType::User))
}

/// Trusted callers only: whether a user chose a password or has a live link.
pub async fn password_token_status(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<PasswordLinkStatus> {
    let GetUser { user } = parse!(params, User);
    PasswordTokenService::status(ctx, user)
        .await
        .or_raise(|| Error::new("failed to get password link status", ErrorType::User))
}

pub async fn password_token_redeem(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<()> {
    let input: RedeemPasswordToken = parse!(params, User);
    PasswordTokenService::redeem(ctx, input)
        .await
        .or_raise(|| Error::new("failed to set password from link", ErrorType::User))
}

pub async fn password_reset_request(
    ctx: &ServiceContext<'_>,
    params: Params<'static>,
) -> Result<()> {
    let input: RequestPasswordReset = parse!(params, User);
    PasswordTokenService::request_reset(ctx, input)
        .await
        .or_raise(|| Error::new("failed to request password reset", ErrorType::User))
}
