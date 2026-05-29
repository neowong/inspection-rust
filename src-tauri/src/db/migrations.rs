use rusqlite::Connection;

/// 运行数据库迁移
///
/// 使用 PRAGMA user_version 跟踪已应用的迁移版本。
/// 版本 1：初始化全部数据库表结构。
pub fn run_migrations(conn: &Connection) -> Result<(), Box<dyn std::error::Error>> {
    // 启用 WAL 模式和外键约束
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;

    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or(0);

    if version < 1 {
        conn.execute_batch(include_str!("../../sql/001_init.sql"))?;
        conn.execute_batch("PRAGMA user_version = 1")?;
    }

    if version < 2 {
        conn.execute_batch(include_str!("../../sql/002_add_deepseek_provider.sql"))?;
        conn.execute_batch("PRAGMA user_version = 2")?;
    }

    Ok(())
}
