//! 数据库迁移管理
//!
//! 定义和管理所有数据库表结构的创建和更新

use crate::backend::errors::{AppError, AppResult};

/// 数据库迁移跟踪表名
const MIGRATIONS_TABLE: &str = "schema_migrations";

/// 运行所有迁移
pub fn run_migrations(conn: &rusqlite::Connection) -> AppResult<()> {
    // 确保迁移跟踪表存在
    ensure_migrations_table(conn)?;

    // 获取已应用的迁移
    let applied: Vec<String> = conn
        .prepare(&format!(
            "SELECT migration_name FROM {} ORDER BY applied_at ASC",
            MIGRATIONS_TABLE
        ))
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get(0))
                .and_then(|rows| rows.collect::<Result<Vec<String>, _>>())
        })
        .map_err(|e| AppError::storage(format!("Failed to query migrations: {}", e)))?;

    // 定义迁移列表
    let migrations: Vec<(&str, &str)> = vec![
        ("001_init", include_str!("../../../migrations/001_init.sql")),
        (
            "002_sessions",
            include_str!("../../../migrations/002_sessions.sql"),
        ),
        (
            "003_skill_evolution",
            include_str!("../../../migrations/003_skill_evolution.sql"),
        ),
        (
            "004_session_messages",
            include_str!("../../../migrations/004_session_messages.sql"),
        ),
    ];

    // 应用未执行的迁移
    for (name, sql) in migrations {
        if !applied.contains(&name.to_string()) {
            apply_migration(conn, name, sql)?;
        }
    }

    Ok(())
}

/// 确保迁移跟踪表存在
fn ensure_migrations_table(conn: &rusqlite::Connection) -> AppResult<()> {
    conn.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                migration_name TEXT NOT NULL UNIQUE,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
            MIGRATIONS_TABLE
        ),
        [],
    )
    .map_err(|e| AppError::storage(format!("Failed to create migrations table: {}", e)))?;

    Ok(())
}

/// 应用单个迁移
fn apply_migration(conn: &rusqlite::Connection, name: &str, sql: &str) -> AppResult<()> {
    // 在事务中执行迁移
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::storage(format!("Failed to begin transaction: {}", e)))?;

    // 执行 SQL
    tx.execute_batch(sql)
        .map_err(|e| AppError::storage(format!("Failed to execute migration {}: {}", name, e)))?;

    // 记录迁移
    tx.execute(
        &format!(
            "INSERT OR IGNORE INTO {} (migration_name) VALUES (?)",
            MIGRATIONS_TABLE
        ),
        [name],
    )
    .map_err(|e| AppError::storage(format!("Failed to record migration {}: {}", name, e)))?;

    // 提交事务
    tx.commit()
        .map_err(|e| AppError::storage(format!("Failed to commit migration {}: {}", name, e)))?;

    tracing::info!("Applied migration: {}", name);
    Ok(())
}
