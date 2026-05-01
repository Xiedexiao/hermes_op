//! Memory records commands

use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::backend::{AppError, Database};

const DEFAULT_MEMORY_SEARCH_LIMIT: usize = 20;
const MAX_MEMORY_SEARCH_LIMIT: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryRecordItem {
    pub id: String,
    pub scope: String,
    pub scope_ref: String,
    pub title: String,
    pub content: String,
    pub source_type: String,
    pub importance: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecordListRequest {
    pub scope: Option<String>,
    pub scope_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecordCreateRequest {
    pub scope: String,
    pub scope_ref: String,
    pub title: String,
    pub content: String,
    pub source_type: String,
    pub importance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecordSearchRequest {
    pub query: String,
    pub limit: Option<usize>,
}

#[tauri::command]
pub fn memory_record_list(
    db: State<'_, Database>,
    request: Option<MemoryRecordListRequest>,
) -> Result<Vec<MemoryRecordItem>, AppError> {
    let (scope, scope_ref) = request
        .map(|value| {
            (
                value.scope.unwrap_or_default(),
                value.scope_ref.unwrap_or_default(),
            )
        })
        .unwrap_or_default();
    list_memory_records(db.inner(), scope.trim(), scope_ref.trim())
}

#[tauri::command]
pub fn memory_record_create(
    db: State<'_, Database>,
    request: MemoryRecordCreateRequest,
) -> Result<MemoryRecordItem, AppError> {
    create_memory_record(db.inner(), request)
}

#[tauri::command]
pub fn memory_record_search(
    db: State<'_, Database>,
    request: MemoryRecordSearchRequest,
) -> Result<Vec<MemoryRecordItem>, AppError> {
    memory_record_search_for_db(db.inner(), request)
}

fn create_memory_record(
    db: &Database,
    request: MemoryRecordCreateRequest,
) -> Result<MemoryRecordItem, AppError> {
    let scope = request.scope.trim().to_string();
    let scope_ref = request.scope_ref.trim().to_string();
    let title = request.title.trim().to_string();
    let content = request.content.trim().to_string();
    let source_type = request.source_type.trim().to_string();
    let importance = request.importance.trim().to_string();

    if scope.is_empty() || scope_ref.is_empty() || title.is_empty() || content.is_empty() {
        return Err(AppError::validation("memory record fields cannot be empty"));
    }

    let item = MemoryRecordItem {
        id: Uuid::new_v4().to_string(),
        scope,
        scope_ref,
        title,
        content,
        source_type,
        importance,
        created_at: Utc::now().to_rfc3339(),
    };

    db.execute(
        "INSERT INTO memory_records (id, scope, scope_ref, title, content, source_type, importance, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            &item.id as &dyn rusqlite::ToSql,
            &item.scope,
            &item.scope_ref,
            &item.title,
            &item.content,
            &item.source_type,
            &item.importance,
            &item.created_at,
        ],
    )?;

    Ok(item)
}

fn list_memory_records(
    db: &Database,
    scope: &str,
    scope_ref: &str,
) -> Result<Vec<MemoryRecordItem>, AppError> {
    let like_scope = scope.to_string();
    let like_scope_ref = scope_ref.to_string();
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, scope, scope_ref, title, content, source_type, importance, created_at
             FROM memory_records
             WHERE (?1 = '' OR scope = ?1)
               AND (?2 = '' OR scope_ref = ?2)
             ORDER BY datetime(created_at) DESC, rowid DESC",
        )?;

        let rows = stmt.query_map([&like_scope, &like_scope_ref], |row| {
            Ok(MemoryRecordItem {
                id: row.get(0)?,
                scope: row.get(1)?,
                scope_ref: row.get(2)?,
                title: row.get(3)?,
                content: row.get(4)?,
                source_type: row.get(5)?,
                importance: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
    })
}

pub fn memory_record_search_for_db(
    db: &Database,
    request: MemoryRecordSearchRequest,
) -> Result<Vec<MemoryRecordItem>, AppError> {
    let query = request.query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let pattern = format!("%{query}%");
    let limit = normalize_search_limit(request.limit) as i64;
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, scope, scope_ref, title, content, source_type, importance, created_at
             FROM memory_records
             WHERE LOWER(title) LIKE ?1
                OR LOWER(content) LIKE ?1
                OR LOWER(source_type) LIKE ?1
                OR LOWER(scope) LIKE ?1
                OR LOWER(scope_ref) LIKE ?1
             ORDER BY datetime(created_at) DESC, rowid DESC
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![pattern, limit], |row| {
            Ok(MemoryRecordItem {
                id: row.get(0)?,
                scope: row.get(1)?,
                scope_ref: row.get(2)?,
                title: row.get(3)?,
                content: row.get(4)?,
                source_type: row.get(5)?,
                importance: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
    })
}

fn normalize_search_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_MEMORY_SEARCH_LIMIT)
        .clamp(1, MAX_MEMORY_SEARCH_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::{
        MemoryRecordCreateRequest, MemoryRecordSearchRequest, create_memory_record,
        list_memory_records, memory_record_search_for_db,
    };
    use crate::backend::Database;

    #[test]
    fn create_and_list_memory_records_for_a_mission() {
        let db = Database::in_memory().expect("db should initialize");
        let created = create_memory_record(
            &db,
            MemoryRecordCreateRequest {
                scope: "mission".to_string(),
                scope_ref: "mission-001".to_string(),
                title: "客户偏好".to_string(),
                content: "偏好周三上午沟通".to_string(),
                source_type: "manual".to_string(),
                importance: "high".to_string(),
            },
        )
        .expect("memory record should create");

        let records =
            list_memory_records(&db, "mission", "mission-001").expect("records should list");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0], created);
    }

    #[test]
    fn memory_record_search_matches_requested_fields_case_insensitively() {
        let db = Database::in_memory().expect("db should initialize");
        create_memory_record(
            &db,
            MemoryRecordCreateRequest {
                scope: "mission".to_string(),
                scope_ref: "mission-001".to_string(),
                title: "VIP Stakeholder".to_string(),
                content: "Prefers Wednesday syncs".to_string(),
                source_type: "gateway:slack".to_string(),
                importance: "high".to_string(),
            },
        )
        .expect("mission memory record should create");
        create_memory_record(
            &db,
            MemoryRecordCreateRequest {
                scope: "session".to_string(),
                scope_ref: "session-abc".to_string(),
                title: "Other".to_string(),
                content: "Unrelated detail".to_string(),
                source_type: "manual".to_string(),
                importance: "low".to_string(),
            },
        )
        .expect("session memory record should create");

        let title_matches = memory_record_search_for_db(
            &db,
            MemoryRecordSearchRequest {
                query: "stakeholder".to_string(),
                limit: None,
            },
        )
        .expect("title matches should search");
        assert_eq!(title_matches.len(), 1);
        assert_eq!(title_matches[0].scope_ref, "mission-001");

        let content_matches = memory_record_search_for_db(
            &db,
            MemoryRecordSearchRequest {
                query: "WEDNESDAY".to_string(),
                limit: None,
            },
        )
        .expect("content matches should search");
        assert_eq!(content_matches.len(), 1);
        assert_eq!(content_matches[0].scope_ref, "mission-001");

        let source_matches = memory_record_search_for_db(
            &db,
            MemoryRecordSearchRequest {
                query: "SLACK".to_string(),
                limit: None,
            },
        )
        .expect("source matches should search");
        assert_eq!(source_matches.len(), 1);
        assert_eq!(source_matches[0].source_type, "gateway:slack");

        let scope_matches = memory_record_search_for_db(
            &db,
            MemoryRecordSearchRequest {
                query: "SESSION".to_string(),
                limit: None,
            },
        )
        .expect("scope matches should search");
        assert_eq!(scope_matches.len(), 1);
        assert_eq!(scope_matches[0].scope, "session");

        let scope_ref_matches = memory_record_search_for_db(
            &db,
            MemoryRecordSearchRequest {
                query: "MISSION-001".to_string(),
                limit: None,
            },
        )
        .expect("scope ref matches should search");
        assert_eq!(scope_ref_matches.len(), 1);
        assert_eq!(scope_ref_matches[0].scope_ref, "mission-001");
    }

    #[test]
    fn memory_record_search_returns_empty_for_blank_queries_and_applies_limit_caps() {
        let db = Database::in_memory().expect("db should initialize");
        for index in 0..60 {
            create_memory_record(
                &db,
                MemoryRecordCreateRequest {
                    scope: "mission".to_string(),
                    scope_ref: format!("mission-{index:03}"),
                    title: format!("Acme memory {index}"),
                    content: "Acme content".to_string(),
                    source_type: "manual".to_string(),
                    importance: "medium".to_string(),
                },
            )
            .expect("memory record should create");
        }

        let blank = memory_record_search_for_db(
            &db,
            MemoryRecordSearchRequest {
                query: "   ".to_string(),
                limit: None,
            },
        )
        .expect("blank search should succeed");
        assert!(blank.is_empty());

        let default_limited = memory_record_search_for_db(
            &db,
            MemoryRecordSearchRequest {
                query: "acme".to_string(),
                limit: None,
            },
        )
        .expect("default limited search should succeed");
        assert_eq!(default_limited.len(), 20);

        let capped = memory_record_search_for_db(
            &db,
            MemoryRecordSearchRequest {
                query: "acme".to_string(),
                limit: Some(200),
            },
        )
        .expect("capped search should succeed");
        assert_eq!(capped.len(), 50);
    }
}
