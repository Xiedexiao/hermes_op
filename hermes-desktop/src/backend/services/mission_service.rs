//! Mission 业务服务

use crate::backend::domain::{
    ContextItemType, CreateKnowledgeChunkInput, CreateKnowledgeSourceInput,
    CreateMissionContextItemInput, CreateMissionInput, KnowledgeSource, Mission,
    MissionContextItem, MissionDetail, MissionListFilter, MissionStatus, UpdateMissionInput,
};
use crate::backend::storage::MissionRepository;
use crate::backend::{AppError, AppResult, Database};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeFeedItem {
    pub id: String,
    pub mission_id: String,
    pub mission_title: String,
    pub source_kind: String,
    pub item_type: String,
    pub title: String,
    pub preview: Option<String>,
    pub source: Option<String>,
    pub path: Option<String>,
    pub created_at: String,
}

pub trait MissionService: Send + Sync {
    fn create(&self, input: CreateMissionInput) -> AppResult<Mission>;
    fn update(&self, input: UpdateMissionInput) -> AppResult<Mission>;
    fn list(&self, filter: MissionListFilter) -> AppResult<Vec<Mission>>;
    fn get(&self, id: &str) -> AppResult<Option<MissionDetail>>;
    fn set_pinned(&self, id: &str, pinned: bool) -> AppResult<Mission>;
    fn set_status(&self, id: &str, status: MissionStatus) -> AppResult<Mission>;
    fn list_knowledge_feed(&self, query: Option<String>) -> AppResult<Vec<KnowledgeFeedItem>>;
    fn list_knowledge_sources(&self, query: Option<String>) -> AppResult<Vec<KnowledgeSource>>;
    fn add_context_item(
        &self,
        input: CreateMissionContextItemInput,
    ) -> AppResult<MissionContextItem>;
}

pub struct MissionServiceImpl {
    repo: MissionRepository,
}

impl MissionServiceImpl {
    pub fn new(db: Database) -> Self {
        Self {
            repo: MissionRepository::new(db),
        }
    }
}

impl MissionService for MissionServiceImpl {
    fn create(&self, input: CreateMissionInput) -> AppResult<Mission> {
        if input.title.trim().is_empty() {
            return Err(AppError::validation("mission title cannot be empty"));
        }

        if input.goal.trim().is_empty() {
            return Err(AppError::validation("mission goal cannot be empty"));
        }

        self.repo.create(input)
    }

    fn update(&self, input: UpdateMissionInput) -> AppResult<Mission> {
        if input.id.trim().is_empty() {
            return Err(AppError::validation("mission id cannot be empty"));
        }
        if input.title.trim().is_empty() {
            return Err(AppError::validation("mission title cannot be empty"));
        }
        if input.goal.trim().is_empty() {
            return Err(AppError::validation("mission goal cannot be empty"));
        }
        if self.repo.get(&input.id)?.is_none() {
            return Err(AppError::storage(format!(
                "mission not found: {}",
                input.id
            )));
        }

        self.repo.update(UpdateMissionInput {
            id: input.id.trim().to_string(),
            title: input.title.trim().to_string(),
            goal: input.goal.trim().to_string(),
            constraints: input
                .constraints
                .into_iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect(),
            success_criteria: input
                .success_criteria
                .into_iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect(),
            priority: input.priority,
        })
    }

    fn list(&self, filter: MissionListFilter) -> AppResult<Vec<Mission>> {
        self.repo.list(filter.normalized())
    }

    fn get(&self, id: &str) -> AppResult<Option<MissionDetail>> {
        let mission = self.repo.get(id)?;
        match mission {
            Some(mission) => {
                let mut context_items = self.repo.list_context_items(&mission.id)?;
                let artifacts = self.repo.list_artifacts(&mission.id)?;
                let artifact_context_items = artifacts
                    .iter()
                    .map(|artifact| MissionContextItem {
                        id: format!("artifact:{}", artifact.id),
                        mission_id: artifact.mission_id.clone(),
                        r#type: ContextItemType::Artifact,
                        title: artifact.title.clone(),
                        content_preview: artifact.mime_type.clone(),
                        source_uri: Some(artifact.path.clone()),
                        pinned: false,
                        created_at: artifact.created_at.clone(),
                    })
                    .collect::<Vec<_>>();
                context_items.extend(artifact_context_items);

                Ok(Some(MissionDetail {
                    context_items,
                    runs: self.repo.list_runs(&mission.id)?,
                    artifacts,
                    mission,
                }))
            }
            None => Ok(None),
        }
    }

    fn set_pinned(&self, id: &str, pinned: bool) -> AppResult<Mission> {
        let normalized = id.trim();
        if normalized.is_empty() {
            return Err(AppError::validation("mission id cannot be empty"));
        }
        if self.repo.get(normalized)?.is_none() {
            return Err(AppError::storage(format!(
                "mission not found: {}",
                normalized
            )));
        }

        self.repo.set_pinned(normalized, pinned)
    }

    fn set_status(&self, id: &str, status: MissionStatus) -> AppResult<Mission> {
        let normalized = id.trim();
        if normalized.is_empty() {
            return Err(AppError::validation("mission id cannot be empty"));
        }
        if self.repo.get(normalized)?.is_none() {
            return Err(AppError::storage(format!(
                "mission not found: {}",
                normalized
            )));
        }

        self.repo.set_status(normalized, status)
    }

    fn list_knowledge_feed(&self, query: Option<String>) -> AppResult<Vec<KnowledgeFeedItem>> {
        let missions = self.repo.list(MissionListFilter {
            query: None,
            status: None,
            limit: Some(10_000),
        })?;
        let normalized_query = query
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty());
        let mut feed = Vec::new();

        for mission in missions {
            let context_items = self.repo.list_context_items(&mission.id)?;
            for item in context_items {
                let feed_item = KnowledgeFeedItem {
                    id: format!("context:{}", item.id),
                    mission_id: mission.id.clone(),
                    mission_title: mission.title.clone(),
                    source_kind: "context_item".to_string(),
                    item_type: context_item_type_label(&item.r#type).to_string(),
                    title: item.title.clone(),
                    preview: item.content_preview.clone(),
                    source: item.source_uri.clone(),
                    path: None,
                    created_at: item.created_at.clone(),
                };
                if matches_knowledge_query(&feed_item, normalized_query.as_deref()) {
                    feed.push(feed_item);
                }
            }

            let artifacts = self.repo.list_artifacts(&mission.id)?;
            for artifact in artifacts {
                let feed_item = KnowledgeFeedItem {
                    id: format!("artifact:{}", artifact.id),
                    mission_id: mission.id.clone(),
                    mission_title: mission.title.clone(),
                    source_kind: "artifact".to_string(),
                    item_type: artifact_type_label(&artifact.r#type).to_string(),
                    title: artifact.title.clone(),
                    preview: artifact.mime_type.clone(),
                    source: Some(artifact.path.clone()),
                    path: Some(artifact.path.clone()),
                    created_at: artifact.created_at.clone(),
                };
                if matches_knowledge_query(&feed_item, normalized_query.as_deref()) {
                    feed.push(feed_item);
                }
            }
        }

        feed.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(feed)
    }

    fn list_knowledge_sources(&self, query: Option<String>) -> AppResult<Vec<KnowledgeSource>> {
        let normalized_query = query
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty());

        let mut sources = self
            .repo
            .list_knowledge_sources()?
            .into_iter()
            .filter(|item| matches_knowledge_source_query(item, normalized_query.as_deref()))
            .collect::<Vec<_>>();
        sources.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(sources)
    }

    fn add_context_item(
        &self,
        input: CreateMissionContextItemInput,
    ) -> AppResult<MissionContextItem> {
        if input.title.trim().is_empty() {
            return Err(AppError::validation("context item title cannot be empty"));
        }

        if self.repo.get(&input.mission_id)?.is_none() {
            return Err(AppError::storage(format!(
                "mission not found: {}",
                input.mission_id
            )));
        }

        let item = self
            .repo
            .create_context_item(CreateMissionContextItemInput {
                mission_id: input.mission_id.trim().to_string(),
                r#type: input.r#type,
                title: input.title.trim().to_string(),
                content_preview: input
                    .content_preview
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                source_uri: input
                    .source_uri
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
            })?;
        let chunks = chunk_knowledge_content(item.content_preview.as_deref())
            .into_iter()
            .map(|content| CreateKnowledgeChunkInput {
                content,
                metadata_json: None,
            })
            .collect::<Vec<_>>();
        let source_uri = item.source_uri.clone().unwrap_or_else(|| {
            format!(
                "knowledge://{}/{}",
                context_item_type_label(&item.r#type),
                item.id
            )
        });
        let index_status = if chunks.is_empty() {
            "pending"
        } else {
            "indexed"
        };

        self.repo
            .create_knowledge_source(CreateKnowledgeSourceInput {
                r#type: context_item_type_label(&item.r#type).to_string(),
                title: item.title.clone(),
                source_uri,
                index_status: index_status.to_string(),
                chunk_count: chunks.len() as i64,
                updated_at: item.created_at.clone(),
                chunks,
            })?;

        Ok(item)
    }
}

fn context_item_type_label(value: &ContextItemType) -> &'static str {
    match value {
        ContextItemType::File => "file",
        ContextItemType::Url => "url",
        ContextItemType::Note => "note",
        ContextItemType::Memory => "memory",
        ContextItemType::KnowledgeResult => "knowledge_result",
        ContextItemType::Artifact => "artifact",
    }
}

fn artifact_type_label(value: &crate::backend::domain::ArtifactType) -> &'static str {
    match value {
        crate::backend::domain::ArtifactType::Markdown => "markdown",
        crate::backend::domain::ArtifactType::Report => "report",
        crate::backend::domain::ArtifactType::Plan => "plan",
        crate::backend::domain::ArtifactType::Json => "json",
        crate::backend::domain::ArtifactType::Text => "text",
        crate::backend::domain::ArtifactType::Image => "image",
        crate::backend::domain::ArtifactType::File => "file",
    }
}

fn matches_knowledge_query(item: &KnowledgeFeedItem, query: Option<&str>) -> bool {
    let Some(query) = query else {
        return true;
    };

    [
        item.mission_title.as_str(),
        item.title.as_str(),
        item.preview.as_deref().unwrap_or(""),
        item.source.as_deref().unwrap_or(""),
        item.path.as_deref().unwrap_or(""),
    ]
    .iter()
    .any(|value| value.to_lowercase().contains(query))
}

fn matches_knowledge_source_query(item: &KnowledgeSource, query: Option<&str>) -> bool {
    let Some(query) = query else {
        return true;
    };

    [
        item.title.as_str(),
        item.r#type.as_str(),
        item.source_uri.as_str(),
        item.index_status.as_str(),
    ]
    .iter()
    .any(|value| value.to_lowercase().contains(query))
}

fn chunk_knowledge_content(content: Option<&str>) -> Vec<String> {
    const MAX_CHARS_PER_CHUNK: usize = 240;

    let Some(content) = content.map(str::trim).filter(|value| !value.is_empty()) else {
        return Vec::new();
    };

    let chars = content.chars().collect::<Vec<_>>();
    chars
        .chunks(MAX_CHARS_PER_CHUNK)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::domain::MissionPriority;
    use serde_json::json;

    #[derive(Debug, PartialEq, Eq)]
    struct PersistedKnowledgeSource {
        id: String,
        r#type: String,
        title: String,
        source_uri: String,
        index_status: String,
        chunk_count: i64,
        updated_at: String,
    }

    fn list_persisted_knowledge_sources(db: &Database) -> Vec<PersistedKnowledgeSource> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, type, title, source_uri, index_status, chunk_count, updated_at
                 FROM knowledge_sources
                 ORDER BY datetime(updated_at) DESC, rowid DESC",
            )?;

            let rows = stmt.query_map([], |row| {
                Ok(PersistedKnowledgeSource {
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
        .expect("knowledge sources should load")
    }

    fn list_persisted_chunks(db: &Database, source_id: &str) -> Vec<(i64, String, Option<String>)> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT chunk_index, content, metadata_json
                 FROM knowledge_chunks
                 WHERE source_id = ?1
                 ORDER BY chunk_index ASC, rowid ASC",
            )?;

            let rows = stmt.query_map([source_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;

            rows.collect::<Result<Vec<_>, _>>()
        })
        .expect("knowledge chunks should load")
    }

    fn sample_input() -> CreateMissionInput {
        CreateMissionInput {
            title: "跟进客户".to_string(),
            goal: "整理客户拜访前的行动方案".to_string(),
            constraints: vec![],
            success_criteria: vec!["生成行动清单".to_string()],
            priority: MissionPriority::Medium,
        }
    }

    #[test]
    fn create_rejects_blank_title() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db);

        let result = service.create(CreateMissionInput {
            title: "   ".to_string(),
            ..sample_input()
        });

        assert!(result.is_err());
    }

    #[test]
    fn create_rejects_blank_goal() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db);

        let result = service.create(CreateMissionInput {
            goal: "".to_string(),
            ..sample_input()
        });

        assert!(result.is_err());
    }

    #[test]
    fn create_and_list_round_trip_through_service() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db);

        let created = service
            .create(sample_input())
            .expect("mission should be created");
        let listed = service
            .list(MissionListFilter::default())
            .expect("missions should list");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert_eq!(listed[0].title, "跟进客户");
    }

    #[test]
    fn list_normalizes_blank_query_through_service() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db);

        service
            .create(sample_input())
            .expect("first mission should be created");
        service
            .create(CreateMissionInput {
                title: "准备续约沟通".to_string(),
                goal: "整理客户续约前的沟通清单".to_string(),
                constraints: vec![],
                success_criteria: vec!["输出待办".to_string()],
                priority: MissionPriority::Medium,
            })
            .expect("second mission should be created");

        let listed = service
            .list(MissionListFilter {
                query: Some("   ".to_string()),
                status: None,
                limit: None,
            })
            .expect("blank query should behave like no filter");

        assert_eq!(listed.len(), 2);
    }

    #[test]
    fn get_returns_detail_shape_with_empty_related_collections() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db);

        let created = service
            .create(sample_input())
            .expect("mission should be created");
        let detail = service
            .get(&created.id)
            .expect("mission detail should be returned");
        let json = serde_json::to_value(detail).expect("detail should serialize");

        assert_eq!(
            json.get("mission").and_then(|value| value.get("id")),
            Some(&json!(created.id))
        );
        assert_eq!(json.get("context_items"), Some(&json!([])));
        assert_eq!(json.get("runs"), Some(&json!([])));
        assert_eq!(json.get("artifacts"), Some(&json!([])));
    }

    #[test]
    fn get_returns_detail_with_runs_and_artifacts() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db.clone());
        let created = service
            .create(sample_input())
            .expect("mission should be created");

        db.execute(
            "INSERT INTO runs (id, mission_id, type, status, summary) VALUES (?, ?, ?, ?, ?)",
            &[
                &"run-1" as &dyn rusqlite::ToSql,
                &created.id,
                &"execution",
                &"completed",
                &"执行完成",
            ],
        )
        .expect("run should insert");

        db.execute(
            "INSERT INTO artifacts (id, mission_id, run_id, type, title, path) VALUES (?, ?, ?, ?, ?, ?)",
            &[
                &"artifact-1" as &dyn rusqlite::ToSql,
                &created.id,
                &Some("run-1".to_string()),
                &"markdown",
                &"客户方案",
                &"/tmp/output.md",
            ],
        )
        .expect("artifact should insert");

        let detail = service
            .get(&created.id)
            .expect("detail should load")
            .expect("detail should exist");

        assert_eq!(detail.runs.len(), 1);
        assert_eq!(detail.runs[0].id, "run-1");
        assert_eq!(detail.artifacts.len(), 1);
        assert_eq!(detail.artifacts[0].id, "artifact-1");
    }

    #[test]
    fn get_aggregates_artifacts_into_detail_context_items() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db.clone());
        let created = service
            .create(sample_input())
            .expect("mission should be created");

        db.execute(
            "INSERT INTO mission_context_items (
                id, mission_id, type, title, content_preview, source_uri, pinned, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"context-1" as &dyn rusqlite::ToSql,
                &created.id,
                &"note",
                &"客户背景",
                &Some("来自最近一次会议".to_string()),
                &Option::<String>::None,
                &1_i64,
                &"2024-01-01T08:00:00Z",
            ],
        )
        .expect("context item should insert");

        db.execute(
            "INSERT INTO artifacts (
                id, mission_id, run_id, type, title, path, mime_type, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"artifact-1" as &dyn rusqlite::ToSql,
                &created.id,
                &Option::<String>::None,
                &"markdown",
                &"拜访方案",
                &"/tmp/visit-plan.md",
                &Some("text/markdown".to_string()),
                &"2024-01-01T09:00:00Z",
            ],
        )
        .expect("artifact should insert");

        let detail = service
            .get(&created.id)
            .expect("detail should load")
            .expect("detail should exist");

        assert_eq!(detail.context_items.len(), 2);
        assert_eq!(detail.context_items[0].id, "context-1");
        assert_eq!(detail.context_items[0].title, "客户背景");
        assert_eq!(detail.context_items[1].id, "artifact:artifact-1");
        assert_eq!(detail.context_items[1].title, "拜访方案");
        assert_eq!(
            detail.context_items[1].source_uri.as_deref(),
            Some("/tmp/visit-plan.md")
        );
        assert_eq!(detail.artifacts.len(), 1);
        assert_eq!(detail.artifacts[0].id, "artifact-1");
    }

    #[test]
    fn get_returns_detail_with_context_items() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db.clone());
        let created = service
            .create(sample_input())
            .expect("mission should be created");

        db.execute(
            "INSERT INTO mission_context_items (
                id, mission_id, type, title, content_preview, source_uri, pinned
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            &[
                &"ctx-1" as &dyn rusqlite::ToSql,
                &created.id,
                &"note",
                &"客户备注",
                &Some("重点关注预算和交付周期".to_string()),
                &Option::<String>::None,
                &0_i64,
            ],
        )
        .expect("context item should insert");

        let detail = service
            .get(&created.id)
            .expect("detail should load")
            .expect("detail should exist");

        assert_eq!(detail.context_items.len(), 1);
        assert_eq!(detail.context_items[0].id, "ctx-1");
        assert_eq!(detail.context_items[0].title, "客户备注");
    }

    #[test]
    fn knowledge_feed_aggregates_context_items_and_artifacts_across_missions() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db.clone());

        let first = service
            .create(sample_input())
            .expect("first mission should be created");
        let second = service
            .create(CreateMissionInput {
                title: "准备季度复盘".to_string(),
                goal: "汇总最近一季度的关键交付".to_string(),
                constraints: vec![],
                success_criteria: vec!["生成复盘文档".to_string()],
                priority: MissionPriority::Low,
            })
            .expect("second mission should be created");

        db.execute(
            "INSERT INTO mission_context_items (
                id, mission_id, type, title, content_preview, source_uri, pinned, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"context-1" as &dyn rusqlite::ToSql,
                &first.id,
                &"note",
                &"客户会议纪要",
                &Some("记录了预算和排期风险".to_string()),
                &Some("manual://meeting-notes".to_string()),
                &0_i64,
                &"2024-01-03T09:00:00Z",
            ],
        )
        .expect("first context item should insert");

        db.execute(
            "INSERT INTO artifacts (
                id, mission_id, run_id, type, title, path, mime_type, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"artifact-1" as &dyn rusqlite::ToSql,
                &second.id,
                &Option::<String>::None,
                &"markdown",
                &"季度复盘草稿",
                &"/tmp/q1-review.md",
                &Some("text/markdown".to_string()),
                &"2024-01-04T10:30:00Z",
            ],
        )
        .expect("artifact should insert");

        let knowledge = service
            .list_knowledge_feed(None)
            .expect("knowledge feed should load");

        assert_eq!(knowledge.len(), 2);
        assert_eq!(knowledge[0].id, "artifact:artifact-1");
        assert_eq!(knowledge[0].mission_id, second.id);
        assert_eq!(knowledge[0].mission_title, "准备季度复盘");
        assert_eq!(knowledge[0].source_kind, "artifact");
        assert_eq!(knowledge[0].item_type, "markdown");
        assert_eq!(knowledge[0].preview.as_deref(), Some("text/markdown"));
        assert_eq!(knowledge[0].path.as_deref(), Some("/tmp/q1-review.md"));
        assert_eq!(knowledge[0].source.as_deref(), Some("/tmp/q1-review.md"));

        assert_eq!(knowledge[1].id, "context:context-1");
        assert_eq!(knowledge[1].mission_id, first.id);
        assert_eq!(knowledge[1].mission_title, "跟进客户");
        assert_eq!(knowledge[1].source_kind, "context_item");
        assert_eq!(knowledge[1].item_type, "note");
        assert_eq!(
            knowledge[1].preview.as_deref(),
            Some("记录了预算和排期风险")
        );
        assert_eq!(
            knowledge[1].source.as_deref(),
            Some("manual://meeting-notes")
        );
        assert_eq!(knowledge[1].path, None);
    }

    #[test]
    fn knowledge_feed_filters_by_query_across_mission_and_item_fields() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db.clone());

        let mission = service
            .create(sample_input())
            .expect("mission should be created");
        let other = service
            .create(CreateMissionInput {
                title: "整理供应商材料".to_string(),
                goal: "输出统一资料包".to_string(),
                constraints: vec![],
                success_criteria: vec!["归档完成".to_string()],
                priority: MissionPriority::Medium,
            })
            .expect("other mission should be created");

        db.execute(
            "INSERT INTO mission_context_items (
                id, mission_id, type, title, content_preview, source_uri, pinned, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"context-1" as &dyn rusqlite::ToSql,
                &mission.id,
                &"url",
                &"ACME 客户门户",
                &Some("预算审批记录".to_string()),
                &Some("https://example.com/acme".to_string()),
                &0_i64,
                &"2024-01-02T12:00:00Z",
            ],
        )
        .expect("context item should insert");

        db.execute(
            "INSERT INTO artifacts (
                id, mission_id, run_id, type, title, path, mime_type, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"artifact-1" as &dyn rusqlite::ToSql,
                &other.id,
                &Option::<String>::None,
                &"report",
                &"供应商汇总",
                &"/tmp/supplier-report.pdf",
                &Some("application/pdf".to_string()),
                &"2024-01-03T12:00:00Z",
            ],
        )
        .expect("artifact should insert");

        let from_mission_title = service
            .list_knowledge_feed(Some("  跟进客户 ".to_string()))
            .expect("mission title query should work");
        let from_source = service
            .list_knowledge_feed(Some("example.com/acme".to_string()))
            .expect("source query should work");
        let from_missing = service
            .list_knowledge_feed(Some("does-not-exist".to_string()))
            .expect("missing query should work");

        assert_eq!(from_mission_title.len(), 1);
        assert_eq!(from_mission_title[0].id, "context:context-1");

        assert_eq!(from_source.len(), 1);
        assert_eq!(from_source[0].id, "context:context-1");

        assert!(from_missing.is_empty());
    }

    #[test]
    fn add_context_item_persists_note_url_and_file_references() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db.clone());
        let mission = service
            .create(sample_input())
            .expect("mission should create");

        let note = service
            .add_context_item(CreateMissionContextItemInput {
                mission_id: mission.id.clone(),
                r#type: ContextItemType::Note,
                title: "客户偏好".to_string(),
                content_preview: Some("偏好周三上午沟通".to_string()),
                source_uri: None,
            })
            .expect("note should create");
        let file = service
            .add_context_item(CreateMissionContextItemInput {
                mission_id: mission.id.clone(),
                r#type: ContextItemType::File,
                title: "报价单".to_string(),
                content_preview: Some("Q2 最新报价".to_string()),
                source_uri: Some("/tmp/quote.xlsx".to_string()),
            })
            .expect("file should create");

        let detail = service
            .get(&mission.id)
            .expect("detail should load")
            .expect("detail should exist");

        assert_eq!(detail.context_items.len(), 2);
        assert_eq!(detail.context_items[0].id, note.id);
        assert_eq!(detail.context_items[1].id, file.id);
        assert_eq!(
            detail.context_items[1].source_uri.as_deref(),
            Some("/tmp/quote.xlsx")
        );
    }

    #[test]
    fn add_context_item_persists_knowledge_source_and_chunks_from_preview_content() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db.clone());
        let mission = service
            .create(sample_input())
            .expect("mission should create");

        let item = service
            .add_context_item(CreateMissionContextItemInput {
                mission_id: mission.id,
                r#type: ContextItemType::Note,
                title: "客户会议纪要".to_string(),
                content_preview: Some("记录预算、范围和时间窗口。".repeat(18)),
                source_uri: None,
            })
            .expect("context item should create");

        let sources = list_persisted_knowledge_sources(&db);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].r#type, "note");
        assert_eq!(sources[0].title, "客户会议纪要");
        assert_eq!(
            sources[0].source_uri,
            format!("knowledge://note/{}", item.id)
        );
        assert_eq!(sources[0].index_status, "indexed");
        assert!(sources[0].chunk_count >= 1);

        let chunks = list_persisted_chunks(&db, &sources[0].id);
        assert_eq!(chunks.len() as i64, sources[0].chunk_count);
        assert!(
            chunks
                .iter()
                .all(|(_, content, _)| !content.trim().is_empty())
        );
    }

    #[test]
    fn add_context_item_persists_source_even_when_no_chunks_are_created() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db.clone());
        let mission = service
            .create(sample_input())
            .expect("mission should create");

        service
            .add_context_item(CreateMissionContextItemInput {
                mission_id: mission.id,
                r#type: ContextItemType::Url,
                title: "ACME 客户门户".to_string(),
                content_preview: None,
                source_uri: Some("https://example.com/acme".to_string()),
            })
            .expect("context item should create");

        let sources = list_persisted_knowledge_sources(&db);
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].r#type, "url");
        assert_eq!(sources[0].source_uri, "https://example.com/acme");
        assert_eq!(sources[0].index_status, "pending");
        assert_eq!(sources[0].chunk_count, 0);

        let chunks = list_persisted_chunks(&db, &sources[0].id);
        assert!(chunks.is_empty());
    }

    #[test]
    fn knowledge_feed_lists_matching_context_items() {
        let db = Database::in_memory().expect("database should initialize");
        let service = MissionServiceImpl::new(db);
        let mission = service
            .create(sample_input())
            .expect("mission should create");

        service
            .add_context_item(CreateMissionContextItemInput {
                mission_id: mission.id.clone(),
                r#type: ContextItemType::File,
                title: "产品简报".to_string(),
                content_preview: Some("包含目标客户、定价和交付排期".to_string()),
                source_uri: Some("/tmp/brief.md".to_string()),
            })
            .expect("file item should create");
        service
            .add_context_item(CreateMissionContextItemInput {
                mission_id: mission.id,
                r#type: ContextItemType::Note,
                title: "内部复盘".to_string(),
                content_preview: Some("只记录关键风险".to_string()),
                source_uri: None,
            })
            .expect("note item should create");

        let feed = service
            .list_knowledge_feed(Some("简报".to_string()))
            .expect("knowledge feed should list");

        assert_eq!(feed.len(), 1);
        assert_eq!(feed[0].title, "产品简报");
        assert_eq!(feed[0].source_kind, "context_item");
        assert_eq!(feed[0].item_type, "file");
        assert_eq!(feed[0].source.as_deref(), Some("/tmp/brief.md"));
        assert!(!feed[0].created_at.is_empty());
    }
}
