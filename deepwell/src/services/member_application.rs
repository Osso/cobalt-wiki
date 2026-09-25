//! Session-scoped site membership applications backed by active site-application relations.

use super::prelude::*;
use crate::models::relation::{self, Entity as Relation};
use crate::models::user::{self, Entity as User};
use crate::services::RelationService;
use crate::services::member_admin::MemberAdminService;
use crate::services::relation::{
    CreateSiteMember, GetSiteMember, RelationObject, RelationReference,
    SiteMemberAccepted, SiteMemberData,
};
use crate::services::role::{GrantUserRoleInput, RoleService};
use crate::types::{RelationObjectType, RelationType, UserType};
use sea_orm::QuerySelect;
use sea_orm::sea_query::LockType;
use std::net::IpAddr;
use time::OffsetDateTime;

const MAX_MESSAGE_CHARS: usize = 2000;

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Application {
    pub message: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Serialize, Debug, Clone)]
pub struct ApplicationStatus {
    pub is_member: bool,
    pub application: Option<Application>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ApplicationEntry {
    pub user_id: i64,
    pub user_name: String,
    pub message: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Deserialize, Debug)]
pub struct SubmitApplication {
    pub message: String,
    pub ip_address: IpAddr,
}

#[derive(Deserialize, Debug)]
pub struct DecideApplication {
    pub user_id: i64,
    pub accept: bool,
    pub ip_address: IpAddr,
}

#[derive(Debug)]
pub struct MemberApplicationService;

impl MemberApplicationService {
    pub async fn get(ctx: &ServiceContext<'_>) -> Result<ApplicationStatus> {
        let (site_id, user_id) = require_user(ctx)?;
        let is_member = is_member(ctx, site_id, user_id).await?;
        let application = pending(ctx, site_id, user_id)
            .await?
            .map(read_application)
            .transpose()?;
        Ok(ApplicationStatus {
            is_member,
            application,
        })
    }

    pub async fn submit(
        ctx: &ServiceContext<'_>,
        input: SubmitApplication,
    ) -> Result<()> {
        let (site_id, user_id) = require_user(ctx)?;
        let message = input.message.trim();
        if message.is_empty() || message.chars().count() > MAX_MESSAGE_CHARS {
            bail!(Error::new(
                "application message must be 1–2000 characters",
                ErrorType::BadRequest
            ));
        }
        // Lock the applicant, not the application: the latter may not exist yet.
        let user = lock_user(ctx, user_id).await?;
        if user.user_type != UserType::Regular {
            bail!(Error::new(
                "only regular accounts may apply",
                ErrorType::PermissionDenied
            ));
        }
        if is_member(ctx, site_id, user_id).await? {
            bail!(Error::new(
                "already a site member",
                ErrorType::SiteMemberExists
            ));
        }
        if pending(ctx, site_id, user_id).await?.is_some() {
            bail!(Error::new(
                "application already pending",
                ErrorType::BadRequest
            ));
        }
        RelationService::create(
            ctx,
            RelationType::SiteApplication,
            RelationObject::Site(site_id),
            RelationObject::User(user_id),
            user_id,
            &serde_json::json!({"message": message}),
        )
        .await?;
        Ok(())
    }

    pub async fn list(ctx: &ServiceContext<'_>) -> Result<Vec<ApplicationEntry>> {
        let site_id = require_admin(ctx).await?;
        let relations = Relation::find()
            .filter(relation::Column::RelationType.eq(RelationType::SiteApplication))
            .filter(relation::Column::DestType.eq(RelationObjectType::Site))
            .filter(relation::Column::DestId.eq(site_id))
            .filter(relation::Column::OverwrittenAt.is_null())
            .filter(relation::Column::DeletedAt.is_null())
            .order_by_asc(relation::Column::CreatedAt)
            .all(ctx.transaction())
            .await
            .or_raise(query_error)?;
        let user_ids: Vec<i64> =
            relations.iter().map(|relation| relation.from_id).collect();
        let users = User::find()
            .filter(user::Column::UserId.is_in(user_ids))
            .all(ctx.transaction())
            .await
            .or_raise(query_error)?;
        let names: std::collections::HashMap<i64, String> = users
            .into_iter()
            .map(|user| (user.user_id, user.name))
            .collect();
        relations
            .into_iter()
            .map(|relation| {
                let user_name = names
                    .get(&relation.from_id)
                    .cloned()
                    .ok_or_raise(query_error)?;
                let user_id = relation.from_id;
                let application = read_application(relation)?;
                Ok(ApplicationEntry {
                    user_id,
                    user_name,
                    message: application.message,
                    created_at: application.created_at,
                })
            })
            .collect()
    }

    pub async fn decide(
        ctx: &ServiceContext<'_>,
        input: DecideApplication,
    ) -> Result<()> {
        let site_id = require_admin(ctx).await?;
        let actor_id = ctx.request().user_id.expect("authorized admin has user ID");
        lock_user(ctx, input.user_id).await?;
        let Some(application) = pending(ctx, site_id, input.user_id).await? else {
            bail!(Error::new("no pending application", ErrorType::BadRequest));
        };
        if input.accept {
            if is_member(ctx, site_id, input.user_id).await? {
                bail!(Error::new(
                    "already a site member",
                    ErrorType::SiteMemberExists
                ));
            }
            RelationService::create_site_member(
                ctx,
                CreateSiteMember {
                    site_id,
                    user_id: input.user_id,
                    metadata: SiteMemberData {
                        accepted: SiteMemberAccepted::Accepted(actor_id),
                    },
                    created_by: actor_id,
                },
                input.ip_address,
            )
            .await?;
            let member_role =
                RoleService::get(ctx, site_id, Reference::Slug("member".into())).await?;
            RoleService::grant_role_to_user(
                ctx,
                GrantUserRoleInput {
                    user_id: input.user_id,
                    role_id: member_role.role_id,
                    site_id,
                    assigning_user_id: actor_id,
                    expires_at: None,
                    ip_address: input.ip_address,
                },
            )
            .await?;
        }
        RelationService::remove(
            ctx,
            RelationReference::Id(application.relation_id),
            actor_id,
        )
        .await?;
        Ok(())
    }
}

fn require_user(ctx: &ServiceContext<'_>) -> Result<(i64, i64)> {
    let request = ctx.request();
    let site_id = request.site_id()?;
    if request
        .session
        .as_ref()
        .is_some_and(|session| session.restricted)
    {
        bail!(Error::new(
            "restricted session",
            ErrorType::PermissionDenied
        ));
    }
    let Some(user_id) = request.user_id else {
        bail!(Error::new("sign in to apply", ErrorType::PermissionDenied));
    };
    Ok((site_id, user_id))
}

async fn require_admin(ctx: &ServiceContext<'_>) -> Result<i64> {
    let (site_id, user_id) = require_user(ctx)?;
    if !MemberAdminService::is_site_admin(ctx, site_id, user_id).await? {
        bail!(Error::new(
            "only site admins review applications",
            ErrorType::PermissionDenied
        ));
    }
    Ok(site_id)
}

async fn lock_user(ctx: &ServiceContext<'_>, user_id: i64) -> Result<user::Model> {
    User::find_by_id(user_id)
        .lock(LockType::Update)
        .one(ctx.transaction())
        .await
        .or_raise(query_error)?
        .ok_or_raise(|| Error::new("applicant not found", ErrorType::UserNotFound))
}

async fn is_member(ctx: &ServiceContext<'_>, site_id: i64, user_id: i64) -> Result<bool> {
    Ok(
        RelationService::get_optional_site_member(
            ctx,
            GetSiteMember { site_id, user_id },
        )
        .await?
        .is_some(),
    )
}

async fn pending(
    ctx: &ServiceContext<'_>,
    site_id: i64,
    user_id: i64,
) -> Result<Option<relation::Model>> {
    RelationService::get_optional(
        ctx,
        RelationReference::Relationship {
            relation_type: RelationType::SiteApplication,
            dest: RelationObject::Site(site_id),
            from: RelationObject::User(user_id),
        },
    )
    .await
}

fn read_application(relation: relation::Model) -> Result<Application> {
    let message = relation.metadata["message"]
        .as_str()
        .ok_or_raise(query_error)?
        .to_owned();
    Ok(Application {
        message,
        created_at: relation.created_at,
    })
}

fn query_error() -> Error {
    Error::new(
        "membership application query failed",
        ErrorType::DatabaseQuery,
    )
}
