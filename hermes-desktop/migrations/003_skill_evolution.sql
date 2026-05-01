-- Skill Evolution Inbox candidates

CREATE TABLE IF NOT EXISTS skill_evolution_candidates (
    id TEXT PRIMARY KEY,
    target_skill_name TEXT,
    action TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    evidence_summary TEXT NOT NULL,
    recommended_change TEXT NOT NULL,
    confidence TEXT NOT NULL DEFAULT 'medium',
    source_refs_json TEXT NOT NULL DEFAULT '[]',
    validation_notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_skill_evolution_candidates_status
    ON skill_evolution_candidates(status);

CREATE INDEX IF NOT EXISTS idx_skill_evolution_candidates_target_skill
    ON skill_evolution_candidates(target_skill_name);
