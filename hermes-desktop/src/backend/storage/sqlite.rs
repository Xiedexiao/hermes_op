//! SQLite 数据库管理
//!
//! 管理数据库连接生命周期和初始化

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use crate::backend::errors::{AppError, AppResult};
use parking_lot::Mutex;

use super::migrations::run_migrations;

/// SQLite 数据库连接包装器（线程安全）
#[derive(Debug, Clone)]
pub struct Database {
    conn: Arc<Mutex<rusqlite::Connection>>,
}

impl Database {
    /// 创建或打开数据库
    pub fn new<P: AsRef<Path>>(path: P) -> AppResult<Self> {
        let path = path.as_ref();

        // 确保父目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::io(format!("Failed to create database directory: {}", e)))?;
        }

        // 打开数据库连接
        let conn = rusqlite::Connection::open(path)
            .map_err(|e| AppError::storage(format!("Failed to open database: {}", e)))?;

        conn.busy_timeout(Duration::from_secs(1))
            .map_err(|e| AppError::storage(format!("Failed to configure busy timeout: {}", e)))?;

        // 启用外键约束
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| AppError::storage(format!("Failed to enable foreign keys: {}", e)))?;

        // 运行迁移
        run_migrations(&conn)?;

        tracing::info!("Database initialized at: {:?}", path);

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// 创建一个内存数据库（用于测试）
    pub fn in_memory() -> AppResult<Self> {
        let conn = rusqlite::Connection::open_in_memory().map_err(|e| {
            AppError::storage(format!("Failed to create in-memory database: {}", e))
        })?;

        conn.busy_timeout(Duration::from_secs(1))
            .map_err(|e| AppError::storage(format!("Failed to configure busy timeout: {}", e)))?;

        // 运行迁移
        run_migrations(&conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// 执行 SQL 查询（返回影响的行数）
    pub fn execute(&self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> AppResult<usize> {
        let conn = self.conn.lock();
        conn.execute(sql, params)
            .map_err(|e| AppError::storage(format!("Failed to execute: {}", e)))
    }

    /// 执行带参数的可切片 SQL
    pub fn execute_slice(&self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> AppResult<usize> {
        self.execute(sql, params)
    }

    /// 执行批量 SQL
    pub fn execute_batch(&self, sql: &str) -> AppResult<()> {
        let conn = self.conn.lock();
        conn.execute_batch(sql)
            .map_err(|e| AppError::storage(format!("Failed to execute batch: {}", e)))
    }

    /// 执行 SQL 查询并返回第一行
    pub fn query_row<T, F>(
        &self,
        sql: &str,
        params: &[&dyn rusqlite::ToSql],
        f: F,
    ) -> Result<T, rusqlite::Error>
    where
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        let conn = self.conn.lock();
        conn.query_row(sql, params, f)
    }

    /// 直接访问底层连接执行自定义读取逻辑
    pub fn with_connection<T, F>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&rusqlite::Connection) -> rusqlite::Result<T>,
    {
        let conn = self.conn.lock();
        f(&conn).map_err(|e| AppError::storage(format!("Database operation failed: {}", e)))
    }

    /// 在事务中执行
    pub fn transaction<T, F>(&self, f: F) -> AppResult<T>
    where
        F: FnOnce(&rusqlite::Transaction<'_>) -> rusqlite::Result<T>,
    {
        let conn = self.conn.lock();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| AppError::storage(format!("Failed to begin transaction: {}", e)))?;
        let result = f(&tx).map_err(|e| AppError::storage(format!("Transaction failed: {}", e)))?;
        tx.commit()
            .map_err(|e| AppError::storage(format!("Failed to commit transaction: {}", e)))?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_database() {
        let db = Database::in_memory().expect("Failed to create in-memory database");
        let conn = db.conn.lock();
        assert!(conn.is_autocommit());
    }

    #[test]
    fn test_core_tables_exist() {
        let db = Database::in_memory().expect("Failed to create in-memory database");

        // 验证核心表存在
        let tables = [
            "app_settings",
            "missions",
            "runs",
            "run_events",
            "skill_evolution_candidates",
        ];
        for table in tables {
            let count: i64 = db
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='{}'",
                        table
                    ),
                    &[],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| panic!("Failed to check table: {}", table));
            assert_eq!(count, 1, "Table {} should exist", table);
        }
    }
}
