//! Mission 数据仓储

use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

use crate::backend::domain::{
    Artifact, ArtifactType, CreateKnowledgeSourceInput, CreateMissionContextItemInput,
    CreateMissionInput, KnowledgeSource, Mission, MissionContextItem, MissionListFilter,
    MissionPriority, MissionStatus, Run, RunStatus, RunType, UpdateMissionInput,
};
use crate::backend::{AppError, AppResult, Database};

#[derive(Clone)]
pub struct MissionRepository {
    db: Database,
}

impl MissionRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create(&self, input: CreateMissionInput) -> AppResult<Mission> {
        let mission = Mission {
            id: Uuid::new_v4().to_string(),
            title: input.title,
            goal: input.goal,
            constraints: input.constraints,
            success_criteria: input.success_criteria,
            status: MissionStatus::Draft,
            priority: input.priority,
            pinned: false,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
            last_activity_at: Utc::now().to_rfc3339(),
        };

        let constraints_json =
            serde_json::to_string(&mission.constraints).map_err(AppError::from_json_error)?;
        let success_criteria_json =
            serde_json::to_string(&mission.success_criteria).map_err(AppError::from_json_error)?;

        self.db.execute(
            "INSERT INTO missions (
                    id, title, goal, constraints_json, success_criteria_json,
                    status, priority, pinned, created_at, updated_at, last_activity_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &mission.id as &dyn rusqlite::ToSql,
                &mission.title,
                &mission.goal,
                &constraints_json,
                &success_criteria_json,
                &mission.status.as_str(),
                &mission.priority.as_str(),
                &(mission.pinned as i64),
                &mission.created_at,
                &mission.updated_at,
                &mission.last_activity_at,
            ],
        )?;

        Ok(mission)
    }

    pub fn list(&self, filter: MissionListFilter) -> AppResult<Vec<Mission>> {
        let filter = filter.normalized();
        let limit = filter.limit.unwrap_or(50) as i64;
        let query = filter.query.unwrap_or_default();
        let status = filter.status.map(|value| value.as_str().to_string());
        let like_query = format!("%{}%", query);

        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, title, goal, constraints_json, success_criteria_json,
                    status, priority, pinned, created_at, updated_at, last_activity_at
                 FROM missions
                 WHERE (?1 = '' OR title LIKE ?2 COLLATE NOCASE OR goal LIKE ?2 COLLATE NOCASE)
                   AND (?3 IS NULL OR status = ?3)
                 ORDER BY pinned DESC, datetime(last_activity_at) DESC, rowid DESC
                 LIMIT ?4",
            )?;

            let rows =
                stmt.query_map(params![query, like_query, status, limit], map_mission_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    pub fn get(&self, id: &str) -> AppResult<Option<Mission>> {
        match self.db.query_row(
            "SELECT
                id, title, goal, constraints_json, success_criteria_json,
                status, priority, pinned, created_at, updated_at, last_activity_at
             FROM missions
             WHERE id = ?1",
            &[&id],
            map_mission_row,
        ) {
            Ok(mission) => Ok(Some(mission)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::storage(format!("Failed to fetch mission: {}", e))),
        }
    }

    pub fn update(&self, input: UpdateMissionInput) -> AppResult<Mission> {
        let updated_at = Utc::now().to_rfc3339();
        let constraints_json =
            serde_json::to_string(&input.constraints).map_err(AppError::from_json_error)?;
        let success_criteria_json =
            serde_json::to_string(&input.success_criteria).map_err(AppError::from_json_error)?;

        self.db.execute(
            "UPDATE missions
             SET title = ?2,
                 goal = ?3,
                 constraints_json = ?4,
                 success_criteria_json = ?5,
                 priority = ?6,
                 updated_at = ?7,
                 last_activity_at = ?7
             WHERE id = ?1",
            &[
                &input.id as &dyn rusqlite::ToSql,
                &input.title,
                &input.goal,
                &constraints_json,
                &success_criteria_json,
                &input.priority.as_str(),
                &updated_at,
            ],
        )?;

        self.get(&input.id)?
            .ok_or_else(|| AppError::storage(format!("mission not found: {}", input.id)))
    }

    pub fn set_pinned(&self, id: &str, pinned: bool) -> AppResult<Mission> {
        let updated_at = Utc::now().to_rfc3339();
        self.db.execute(
            "UPDATE missions
             SET pinned = ?2,
                 updated_at = ?3,
                 last_activity_at = ?3
             WHERE id = ?1",
            &[&id as &dyn rusqlite::ToSql, &(pinned as i64), &updated_at],
        )?;

        self.get(id)?
            .ok_or_else(|| AppError::storage(format!("mission not found: {}", id)))
    }

    pub fn set_status(&self, id: &str, status: MissionStatus) -> AppResult<Mission> {
        let updated_at = Utc::now().to_rfc3339();
        self.db.execute(
            "UPDATE missions
             SET status = ?2,
                 updated_at = ?3,
                 last_activity_at = ?3
             WHERE id = ?1",
            &[&id as &dyn rusqlite::ToSql, &status.as_str(), &updated_at],
        )?;

        self.get(id)?
            .ok_or_else(|| AppError::storage(format!("mission not found: {}", id)))
    }

    pub fn list_context_items(&self, mission_id: &str) -> AppResult<Vec<MissionContextItem>> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, mission_id, type, title, content_preview, source_uri, pinned, created_at
                 FROM mission_context_items
                 WHERE mission_id = ?1
                 ORDER BY datetime(created_at) ASC, rowid ASC",
            )?;

            let rows = stmt.query_map(params![mission_id], |row| {
                Ok(MissionContextItem {
                    id: row.get(0)?,
                    mission_id: row.get(1)?,
                    r#type: crate::backend::domain::ContextItemType::from_key(
                        &row.get::<_, String>(2)?,
                    ),
                    title: row.get(3)?,
                    content_preview: row.get(4)?,
                    source_uri: row.get(5)?,
                    pinned: row.get::<_, i64>(6)? != 0,
                    created_at: row.get(7)?,
                })
            })?;

            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    pub fn create_context_item(
        &self,
        input: CreateMissionContextItemInput,
    ) -> AppResult<MissionContextItem> {
        let item = MissionContextItem {
            id: Uuid::new_v4().to_string(),
            mission_id: input.mission_id,
            r#type: input.r#type,
            title: input.title,
            content_preview: input.content_preview,
            source_uri: input.source_uri,
            pinned: false,
            created_at: Utc::now().to_rfc3339(),
        };

        self.db.execute(
            "INSERT INTO mission_context_items (
                id, mission_id, type, title, content_preview, source_uri, pinned, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &item.id as &dyn rusqlite::ToSql,
                &item.mission_id,
                &context_item_type_to_str(&item.r#type),
                &item.title,
                &item.content_preview,
                &item.source_uri,
                &(item.pinned as i64),
                &item.created_at,
            ],
        )?;

        Ok(item)
    }

    pub fn create_knowledge_source(
        &self,
        input: CreateKnowledgeSourceInput,
    ) -> AppResult<KnowledgeSource> {
        let source = KnowledgeSource {
            id: Uuid::new_v4().to_string(),
            r#type: input.r#type,
            title: input.title,
            source_uri: input.source_uri,
            index_status: input.index_status,
            chunk_count: input.chunk_count,
            updated_at: input.updated_at.clone(),
        };
        let created_at = input.updated_at;

        self.db.execute(
            "INSERT INTO knowledge_sources (
                id, type, title, source_uri, index_status, chunk_count, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &source.id as &dyn rusqlite::ToSql,
                &source.r#type,
                &source.title,
                &source.source_uri,
                &source.index_status,
                &source.chunk_count,
                &created_at,
                &source.updated_at,
            ],
        )?;

        for (chunk_index, chunk) in input.chunks.iter().enumerate() {
            let chunk_id = Uuid::new_v4().to_string();
            self.db.execute(
                "INSERT INTO knowledge_chunks (
                    id, source_id, chunk_index, content, metadata_json
                ) VALUES (?, ?, ?, ?, ?)",
                &[
                    &chunk_id as &dyn rusqlite::ToSql,
                    &source.id,
                    &(chunk_index as i64),
                    &chunk.content,
                    &chunk.metadata_json,
                ],
            )?;
        }

        Ok(source)
    }

    pub fn list_knowledge_sources(&self) -> AppResult<Vec<KnowledgeSource>> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, type, title, source_uri, index_status, chunk_count, updated_at
                 FROM knowledge_sources
                 ORDER BY datetime(updated_at) DESC, rowid DESC",
            )?;

            let rows = stmt.query_map([], |row| {
                Ok(KnowledgeSource {
                    id: row.get(0)?,
                    r#type: row.get(1)?,
                    title: row.get(2)?,
                    source_uri: row.get(3)?,
                    index_status: row.get(4)?,
                    chunk_count: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;

            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    pub fn list_runs(&self, mission_id: &str) -> AppResult<Vec<Run>> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, mission_id, type, status, started_at, finished_at, summary, error_message
                 FROM runs
                 WHERE mission_id = ?1
                 ORDER BY rowid ASC",
            )?;

            let rows = stmt.query_map(params![mission_id], |row| {
                Ok(Run {
                    id: row.get(0)?,
                    mission_id: row.get(1)?,
                    r#type: RunType::from_key(&row.get::<_, String>(2)?),
                    status: RunStatus::from_key(&row.get::<_, String>(3)?),
                    started_at: row.get(4)?,
                    finished_at: row.get(5)?,
                    summary: row.get(6)?,
                    error_message: row.get(7)?,
                })
            })?;

            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    pub fn list_artifacts(&self, mission_id: &str) -> AppResult<Vec<Artifact>> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, mission_id, run_id, type, title, path, mime_type, created_at
                 FROM artifacts
                 WHERE mission_id = ?1
                 ORDER BY rowid ASC",
            )?;

            let rows = stmt.query_map(params![mission_id], |row| {
                Ok(Artifact {
                    id: row.get(0)?,
                    mission_id: row.get(1)?,
                    run_id: row.get(2)?,
                    r#type: ArtifactType::from_key(&row.get::<_, String>(3)?),
                    title: row.get(4)?,
                    path: row.get(5)?,
                    mime_type: row.get(6)?,
                    created_at: row.get(7)?,
                })
            })?;

            rows.collect::<Result<Vec<_>, _>>()
        })
    }
}

fn map_mission_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Mission> {
    let constraints_json: String = row.get(3)?;
    let success_criteria_json: String = row.get(4)?;
    let constraints = serde_json::from_str(&constraints_json).unwrap_or_default();
    let success_criteria = serde_json::from_str(&success_criteria_json).unwrap_or_default();

    Ok(Mission {
        id: row.get(0)?,
        title: row.get(1)?,
        goal: row.get(2)?,
        constraints,
        success_criteria,
        status: MissionStatus::from_key(&row.get::<_, String>(5)?),
        priority: MissionPriority::from_key(&row.get::<_, String>(6)?),
        pinned: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        last_activity_at: row.get(10)?,
    })
}

fn context_item_type_to_str(value: &crate::backend::domain::ContextItemType) -> &'static str {
    match value {
        crate::backend::domain::ContextItemType::File => "file",
        crate::backend::domain::ContextItemType::Url => "url",
        crate::backend::domain::ContextItemType::Note => "note",
        crate::backend::domain::ContextItemType::Memory => "memory",
        crate::backend::domain::ContextItemType::KnowledgeResult => "knowledge_result",
        crate::backend::domain::ContextItemType::Artifact => "artifact",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::domain::{MissionPriority, MissionStatus, UpdateMissionInput};

    fn sample_input() -> CreateMissionInput {
        CreateMissionInput {
            title: "准备客户拜访方案".to_string(),
            goal: "基于现有资料生成明日拜访方案".to_string(),
            constraints: vec!["不得对外发送邮件".to_string()],
            success_criteria: vec!["生成 Markdown 方案".to_string()],
            priority: MissionPriority::High,
        }
    }

    #[test]
    fn create_mission_persists_and_returns_domain_object() {
        let db = Database::in_memory().expect("in-memory database should initialize");
        let repo = MissionRepository::new(db);

        let mission = repo
            .create(sample_input())
            .expect("mission should be created");

        assert_eq!(mission.title, "准备客户拜访方案");
        assert_eq!(mission.goal, "基于现有资料生成明日拜访方案");
        assert_eq!(mission.constraints, vec!["不得对外发送邮件"]);
        assert_eq!(mission.success_criteria, vec!["生成 Markdown 方案"]);
        assert_eq!(mission.priority, MissionPriority::High);
        assert_eq!(mission.status, MissionStatus::Draft);
        assert!(!mission.id.is_empty());
    }

    #[test]
    fn create_context_item_persists_mission_context_item() {
        let db = Database::in_memory().expect("in-memory database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo.create(sample_input()).expect("mission should create");

        let item = repo
            .create_context_item(CreateMissionContextItemInput {
                mission_id: mission.id.clone(),
                r#type: crate::backend::domain::ContextItemType::Url,
                title: "客户门户".to_string(),
                content_preview: Some("记录预算审批".to_string()),
                source_uri: Some("https://example.com/acme".to_string()),
            })
            .expect("context item should create");

        let listed = repo
            .list_context_items(&mission.id)
            .expect("context items should list");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, item.id);
        assert_eq!(listed[0].title, "客户门户");
        assert_eq!(
            listed[0].source_uri.as_deref(),
            Some("https://example.com/acme")
        );
    }

    #[test]
    fn list_missions_returns_newest_first_and_respects_limit() {
        let db = Database::in_memory().expect("in-memory database should initialize");
        let repo = MissionRepository::new(db);

        let first = repo.create(CreateMissionInput {
            title: "第一个任务".to_string(),
            ..sample_input()
        });
        assert!(first.is_ok());

        let second = repo.create(CreateMissionInput {
            title: "第二个任务".to_string(),
            ..sample_input()
        });
        assert!(second.is_ok());

        let all = repo
            .list(MissionListFilter {
                query: None,
                status: None,
                limit: None,
            })
            .expect("listing missions should work");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].title, "第二个任务");
        assert_eq!(all[1].title, "第一个任务");

        let limited = repo
            .list(MissionListFilter {
                query: None,
                status: None,
                limit: Some(1),
            })
            .expect("listing missions with limit should work");
        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].title, "第二个任务");
    }

    #[test]
    fn list_missions_treats_blank_query_as_no_filter() {
        let db = Database::in_memory().expect("in-memory database should initialize");
        let repo = MissionRepository::new(db);

        repo.create(CreateMissionInput {
            title: "客户复盘".to_string(),
            ..sample_input()
        })
        .expect("first mission should be created");

        repo.create(CreateMissionInput {
            title: "供应商谈判".to_string(),
            ..sample_input()
        })
        .expect("second mission should be created");

        let listed = repo
            .list(MissionListFilter {
                query: Some("   ".to_string()),
                status: None,
                limit: None,
            })
            .expect("blank query should behave like no filter");

        assert_eq!(listed.len(), 2);
    }

    #[test]
    fn list_missions_trims_query_before_matching() {
        let db = Database::in_memory().expect("in-memory database should initialize");
        let repo = MissionRepository::new(db);

        repo.create(CreateMissionInput {
            title: "客户跟进".to_string(),
            goal: "整理 ACME 客户拜访方案".to_string(),
            ..sample_input()
        })
        .expect("mission should be created");

        let listed = repo
            .list(MissionListFilter {
                query: Some("  ACME  ".to_string()),
                status: None,
                limit: None,
            })
            .expect("trimmed query should match goal");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].title, "客户跟进");
    }

    #[test]
    fn get_mission_returns_none_for_missing_and_some_for_existing() {
        let db = Database::in_memory().expect("in-memory database should initialize");
        let repo = MissionRepository::new(db);

        assert!(
            repo.get("missing-id")
                .expect("missing lookup should work")
                .is_none()
        );

        let mission = repo
            .create(sample_input())
            .expect("mission should be created");

        let fetched = repo
            .get(&mission.id)
            .expect("existing lookup should work")
            .expect("mission should exist");

        assert_eq!(fetched.id, mission.id);
        assert_eq!(fetched.title, mission.title);
    }

    #[test]
    fn update_mission_rewrites_editable_fields_and_updates_activity_timestamps() {
        let db = Database::in_memory().expect("in-memory database should initialize");
        let repo = MissionRepository::new(db);
        let mission = repo
            .create(sample_input())
            .expect("mission should be created");

        let updated = repo
            .update(UpdateMissionInput {
                id: mission.id.clone(),
                title: "更新后的任务标题".to_string(),
                goal: "更新后的任务目标".to_string(),
                constraints: vec!["先整理线索".to_string(), "不要直接外呼".to_string()],
                success_criteria: vec!["形成执行方案".to_string()],
                priority: MissionPriority::Low,
            })
            .expect("mission should update");

        assert_eq!(updated.id, mission.id);
        assert_eq!(updated.title, "更新后的任务标题");
        assert_eq!(updated.goal, "更新后的任务目标");
        assert_eq!(
            updated.constraints,
            vec!["先整理线索".to_string(), "不要直接外呼".to_string()]
        );
        assert_eq!(updated.success_criteria, vec!["形成执行方案".to_string()]);
        assert_eq!(updated.priority, MissionPriority::Low);
        assert_ne!(updated.updated_at, mission.updated_at);
        assert_ne!(updated.last_activity_at, mission.last_activity_at);

        let persisted = repo
            .get(&mission.id)
            .expect("lookup should work")
            .expect("mission should exist");
        assert_eq!(persisted.title, "更新后的任务标题");
        assert_eq!(persisted.priority, MissionPriority::Low);
    }

    #[test]
    fn set_pinned_and_status_support_pin_archive_and_status_filtering() {
        let db = Database::in_memory().expect("in-memory database should initialize");
        let repo = MissionRepository::new(db);
        let draft = repo
            .create(CreateMissionInput {
                title: "Draft mission".to_string(),
                ..sample_input()
            })
            .expect("draft mission should create");
        let researching = repo
            .create(CreateMissionInput {
                title: "Research mission".to_string(),
                ..sample_input()
            })
            .expect("research mission should create");

        let pinned = repo
            .set_pinned(&draft.id, true)
            .expect("pin should update mission");
        let archived = repo
            .set_status(&draft.id, MissionStatus::Archived)
            .expect("archive should update mission");
        let moved = repo
            .set_status(&researching.id, MissionStatus::Researching)
            .expect("status should update mission");

        assert!(pinned.pinned);
        assert_eq!(archived.status, MissionStatus::Archived);
        assert_eq!(moved.status, MissionStatus::Researching);

        let researching_only = repo
            .list(MissionListFilter {
                query: None,
                status: Some(MissionStatus::Researching),
                limit: None,
            })
            .expect("status-filtered listing should work");
        assert_eq!(researching_only.len(), 1);
        assert_eq!(researching_only[0].id, researching.id);

        let archived_only = repo
            .list(MissionListFilter {
                query: None,
                status: Some(MissionStatus::Archived),
                limit: None,
            })
            .expect("archived listing should work");
        assert_eq!(archived_only.len(), 1);
        assert_eq!(archived_only[0].id, draft.id);
        assert!(archived_only[0].pinned);
    }
}
