/*
 * services/job/structs.rs
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

use crate::services::page_revision::RerenderType;
use crate::types::PageId;

/// Jobs queued before the `depth` field was removed still carry it;
/// serde ignores it.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case", tag = "job", content = "data")]
pub enum Job {
    RerenderPage { id: PageId, r#type: RerenderType },
    PruneSessions,
    PrunePendingUploads,
    PruneText,
    NameChangeRefill,
    LiftExpiredPunishments,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rerender_job_queued_with_depth_still_deserializes() {
        let queued = r#"{"job":"rerender_page","data":{"id":{"site_id":6,"category_id":7,"page_id":8},"depth":3,"type":"nav"}}"#;
        let job: Job = serde_json::from_str(queued).expect("old rerender job payload");
        let Job::RerenderPage { id, r#type } = job else {
            panic!("not a rerender job: {job:?}");
        };
        assert_eq!((id.site_id, id.category_id, id.page_id), (6, 7, 8));
        assert_eq!(r#type, RerenderType::NavigationOnly);
    }
}
