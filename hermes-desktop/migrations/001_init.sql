-- Hermes Operator 初始数据库结构
-- 创建时间: 2024

-- ============================================
-- 1. 应用设置表
-- ============================================
CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================
-- 2. 任务表
-- ============================================
CREATE TABLE IF NOT EXISTS missions (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    goal TEXT NOT NULL,
    constraints_json TEXT NOT NULL DEFAULT '[]',
    success_criteria_json TEXT NOT NULL DEFAULT '[]',
    status TEXT NOT NULL DEFAULT 'draft',
    priority TEXT NOT NULL DEFAULT 'medium',
    pinned INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_activity_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_missions_status ON missions(status);
CREATE INDEX IF NOT EXISTS idx_missions_last_activity_at ON missions(last_activity_at);

-- ============================================
-- 3. 任务上下文项表
-- ============================================
CREATE TABLE IF NOT EXISTS mission_context_items (
    id TEXT PRIMARY KEY,
    mission_id TEXT NOT NULL,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    content_preview TEXT,
    source_uri TEXT,
    pinned INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (mission_id) REFERENCES missions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_mission_context_items_mission_id ON mission_context_items(mission_id);

-- ============================================
-- 4. 运行记录表
-- ============================================
CREATE TABLE IF NOT EXISTS runs (
    id TEXT PRIMARY KEY,
    mission_id TEXT NOT NULL,
    type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    started_at TEXT,
    finished_at TEXT,
    summary TEXT,
    error_message TEXT,
    FOREIGN KEY (mission_id) REFERENCES missions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_runs_mission_id ON runs(mission_id);
CREATE INDEX IF NOT EXISTS idx_runs_status ON runs(status);

-- ============================================
-- 5. 运行事件表
-- ============================================
CREATE TABLE IF NOT EXISTS run_events (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    mission_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    message TEXT NOT NULL,
    payload_json TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE,
    FOREIGN KEY (mission_id) REFERENCES missions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_run_events_run_id ON run_events(run_id);
CREATE INDEX IF NOT EXISTS idx_run_events_mission_id ON run_events(mission_id);

-- ============================================
-- 6. 产物表
-- ============================================
CREATE TABLE IF NOT EXISTS artifacts (
    id TEXT PRIMARY KEY,
    mission_id TEXT NOT NULL,
    run_id TEXT,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    path TEXT NOT NULL,
    mime_type TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (mission_id) REFERENCES missions(id) ON DELETE CASCADE,
    FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_artifacts_mission_id ON artifacts(mission_id);
CREATE INDEX IF NOT EXISTS idx_artifacts_run_id ON artifacts(run_id);

-- ============================================
-- 7. 知识源表
-- ============================================
CREATE TABLE IF NOT EXISTS knowledge_sources (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    title TEXT NOT NULL,
    source_uri TEXT NOT NULL,
    index_status TEXT NOT NULL DEFAULT 'pending',
    chunk_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_knowledge_sources_type ON knowledge_sources(type);
CREATE INDEX IF NOT EXISTS idx_knowledge_sources_index_status ON knowledge_sources(index_status);

-- ============================================
-- 8. 知识块表
-- ============================================
CREATE TABLE IF NOT EXISTS knowledge_chunks (
    id TEXT PRIMARY KEY,
    source_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    content TEXT NOT NULL,
    metadata_json TEXT,
    FOREIGN KEY (source_id) REFERENCES knowledge_sources(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_knowledge_chunks_source_id ON knowledge_chunks(source_id);

-- ============================================
-- 9. 记忆记录表
-- ============================================
CREATE TABLE IF NOT EXISTS memory_records (
    id TEXT PRIMARY KEY,
    scope TEXT NOT NULL,
    scope_ref TEXT NOT NULL,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    source_type TEXT NOT NULL,
    importance TEXT NOT NULL DEFAULT 'medium',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_memory_records_scope ON memory_records(scope);
CREATE INDEX IF NOT EXISTS idx_memory_records_scope_ref ON memory_records(scope_ref);

-- ============================================
-- 10. 场景运行表
-- ============================================
CREATE TABLE IF NOT EXISTS scenario_runs (
    id TEXT PRIMARY KEY,
    mission_id TEXT NOT NULL,
    baseline TEXT NOT NULL,
    options_json TEXT NOT NULL DEFAULT '[]',
    recommendation TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (mission_id) REFERENCES missions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_scenario_runs_mission_id ON scenario_runs(mission_id);

-- ============================================
-- 11. 审议步骤表
-- ============================================
CREATE TABLE IF NOT EXISTS council_steps (
    id TEXT PRIMARY KEY,
    mission_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    role TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    input_summary TEXT,
    output_summary TEXT,
    review_note TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (mission_id) REFERENCES missions(id) ON DELETE CASCADE,
    FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_council_steps_mission_id ON council_steps(mission_id);
CREATE INDEX IF NOT EXISTS idx_council_steps_run_id ON council_steps(run_id);

-- ============================================
-- 12. 执行步骤表
-- ============================================
CREATE TABLE IF NOT EXISTS execution_steps (
    id TEXT PRIMARY KEY,
    mission_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    title TEXT NOT NULL,
    mode TEXT NOT NULL,
    risk_level TEXT NOT NULL DEFAULT 'low',
    status TEXT NOT NULL DEFAULT 'pending',
    input_payload TEXT,
    output_summary TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (mission_id) REFERENCES missions(id) ON DELETE CASCADE,
    FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_execution_steps_mission_id ON execution_steps(mission_id);
CREATE INDEX IF NOT EXISTS idx_execution_steps_run_id ON execution_steps(run_id);
