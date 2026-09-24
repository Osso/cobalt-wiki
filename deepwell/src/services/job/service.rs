/*
 * services/job/service.rs
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

use super::prelude::*;
use crate::services::page_revision::RerenderType;
use crate::types::PageId;
use redis::AsyncCommands;
use rsmq_async::{Rsmq, RsmqConnection};
use std::time::Duration;

pub const JOB_QUEUE_NAME: &str = "job";

/// How long messages, after being delivered, cannot be delivered to another consumer.
///
/// This feature is a part of job queues to prevent a job from being run twice by
/// two different consumers. Because a job which fails won't report back to the queue,
/// so you cannot tell the difference between a job which is in progress vs a job that failed.
///
/// The way we resolve this is setting a time limit for "process time", which is the designated
/// period a job is allowed to run. If a job takes longer than that, then we assume it failed or
/// died. This risks a false positive of still-running jobs, but as long as this time is well
/// above what a job should take to run this risk is minimal.
pub const JOB_QUEUE_PROCESS_TIME: Option<Duration> = Some(Duration::from_secs(30));

/// How long to wait before messages are delivered to consumers.
pub const JOB_QUEUE_DELAY: Option<Duration> = None;

/// The maximum size, in bytes, that a job payload is allowed to be
///
/// Presently, our jobs are mostly unit types, and the biggest variant
/// is composed of three integers, so this is more than large enough.
/// If larger jobs become a thing in the future, this may need to be updated.
///
/// (But as a general code principle there shouldn't be huge jobs, they should
/// just use IDs and references to items in the database.)
pub const JOB_QUEUE_MAXIMUM_SIZE: Option<i64> = Some(1024);

#[derive(Debug)]
pub struct JobService;

impl JobService {
    pub async fn queue_job(
        ctx: &ServiceContext<'_>,
        job: &Job,
        delay: Option<Duration>,
    ) -> Result<()> {
        let mut rsmq = ctx.rsmq();
        Self::queue_job_inner(&mut rsmq, job, delay).await
    }

    pub async fn queue_job_inner(
        rsmq: &mut Rsmq,
        job: &Job,
        delay: Option<Duration>,
    ) -> Result<()> {
        info!("Queuing job {job:?} (delay {delay:?})");

        let make_error = || {
            Error::new(
                format!(
                    "failed to queue job to RSMQ: {:#?} (delay {:?})",
                    job, delay,
                ),
                ErrorType::Job,
            )
        };

        let payload = serde_json::to_vec(job).or_raise(make_error)?;
        rsmq.send_message(JOB_QUEUE_NAME, payload, delay)
            .await
            .or_raise(make_error)?;

        Ok(())
    }

    /// Queues a dependent page for being rerendered soon.
    ///
    /// The job rerenders the page standalone: the change that made it
    /// stale already queued every other page it affects.
    ///
    /// # Arguments
    /// | Argument  | Description |
    /// |-----------|-------------|
    /// | `id` | The page to rerender. |
    pub async fn queue_rerender_page(ctx: &ServiceContext<'_>, id: PageId) -> Result<()> {
        Self::queue_rerender(ctx, id, RerenderType::Standalone).await
    }

    /// Queues a page's navigation page data for rerendering soon.
    ///
    /// # Arguments
    /// Same as `queue_rerender_page()`.
    pub async fn queue_rerender_nav_page(
        ctx: &ServiceContext<'_>,
        id: PageId,
    ) -> Result<()> {
        Self::queue_rerender(ctx, id, RerenderType::NavigationOnly).await
    }

    /// Queues a rerender job unless the same one is already pending.
    ///
    /// A pending job has not started yet, so it will read the change that is
    /// queuing it now. Its marker is removed when a worker starts it.
    async fn queue_rerender(
        ctx: &ServiceContext<'_>,
        id: PageId,
        rerender_type: RerenderType,
    ) -> Result<()> {
        let key = pending_rerender_key(id, rerender_type);
        let make_error = || {
            Error::new(
                format!("failed to mark rerender job pending: {key}"),
                ErrorType::Job,
            )
        };

        let mut redis = ctx.redis();
        let marked: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg(1)
            .arg("NX")
            .arg("EX")
            .arg(PENDING_RERENDER_EXPIRY_SECS)
            .query_async(&mut redis)
            .await
            .or_raise(make_error)?;
        if marked.is_none() {
            debug!("Rerender job already pending: {key}");
            return Ok(());
        }

        debug!(
            "Queuing {rerender_type:?} rerender for page ID {} and site ID {}",
            id.page_id, id.site_id,
        );
        let job = Job::RerenderPage {
            id,
            r#type: rerender_type,
        };
        if let Err(error) = Self::queue_job(ctx, &job, None).await {
            let _: () = redis.del(&key).await.or_raise(make_error)?;
            return Err(error);
        }

        Ok(())
    }

    /// Called by a worker when it starts a rerender job, so that later
    /// changes queue the page again.
    pub async fn start_rerender_job(
        ctx: &ServiceContext<'_>,
        id: PageId,
        rerender_type: RerenderType,
    ) -> Result<()> {
        let key = pending_rerender_key(id, rerender_type);
        let _: () = ctx.redis().del(&key).await.or_raise(|| {
            Error::new(
                format!("failed to clear pending rerender job: {key}"),
                ErrorType::Job,
            )
        })?;
        Ok(())
    }
}

/// How long a pending-rerender marker lives. A worker removes it when the job
/// starts; the expiry only frees a page whose job was lost.
const PENDING_RERENDER_EXPIRY_SECS: u64 = 3600;

fn pending_rerender_key(id: PageId, rerender_type: RerenderType) -> String {
    format!("job:rerender-pending:{}:{rerender_type:?}", id.page_id)
}
