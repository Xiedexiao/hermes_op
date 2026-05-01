use hermes_desktop::backend::{
    ContextItemType, CreateMissionContextItemInput, CreateMissionInput, Database, MissionPriority,
    MissionService, MissionServiceImpl,
};

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

fn sample_input() -> CreateMissionInput {
    CreateMissionInput {
        title: "跟进客户".to_string(),
        goal: "整理客户拜访前的行动方案".to_string(),
        constraints: vec![],
        success_criteria: vec!["生成行动清单".to_string()],
        priority: MissionPriority::Medium,
    }
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

fn list_persisted_chunks(db: &Database, source_id: &str) -> Vec<(i64, String)> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT chunk_index, content
             FROM knowledge_chunks
             WHERE source_id = ?1
             ORDER BY chunk_index ASC, rowid ASC",
        )?;

        let rows = stmt.query_map([source_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()
    })
    .expect("knowledge chunks should load")
}

#[test]
fn knowledge_import_persists_source_and_chunks() {
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
    assert!(chunks.iter().all(|(_, content)| !content.trim().is_empty()));
}

#[test]
fn knowledge_import_persists_pending_source_without_chunks() {
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
    assert!(list_persisted_chunks(&db, &sources[0].id).is_empty());
}

#[test]
fn knowledge_source_list_returns_filtered_metadata() {
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

    let sources = service
        .list_knowledge_sources(Some("简报".to_string()))
        .expect("knowledge sources should list");

    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].title, "产品简报");
    assert_eq!(sources[0].r#type, "file");
    assert_eq!(sources[0].source_uri, "/tmp/brief.md");
    assert_eq!(sources[0].index_status, "indexed");
    assert!(sources[0].chunk_count >= 1);
    assert!(!sources[0].updated_at.is_empty());
}
