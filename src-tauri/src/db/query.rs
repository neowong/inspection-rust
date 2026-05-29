use rusqlite::{Connection, ToSql};

/// 查询多条记录
///
/// # Arguments
/// * `conn` - 数据库连接
/// * `sql` - SQL 查询语句
/// * `params` - 查询参数
/// * `f` - 行映射函数，将 `Row` 转换为目标类型
pub fn query_all<T, F>(
    conn: &Connection,
    sql: &str,
    params: &[&dyn ToSql],
    f: F,
) -> Result<Vec<T>, String>
where
    F: Fn(&rusqlite::Row) -> rusqlite::Result<T>,
{
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt.query_map(params, f).map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| e.to_string())?);
    }
    Ok(results)
}

/// 查询单条记录
///
/// # Arguments
/// * `conn` - 数据库连接
/// * `sql` - SQL 查询语句
/// * `params` - 查询参数
/// * `f` - 行映射函数，将 `Row` 转换为目标类型
pub fn query_one<T, F>(
    conn: &Connection,
    sql: &str,
    params: &[&dyn ToSql],
    f: F,
) -> Result<Option<T>, String>
where
    F: Fn(&rusqlite::Row) -> rusqlite::Result<T>,
{
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params).map_err(|e| e.to_string())?;
    match rows.next().map_err(|e| e.to_string())? {
        Some(row) => Ok(Some(f(row).map_err(|e| e.to_string())?)),
        None => Ok(None),
    }
}

/// 查询记录数
///
/// # Arguments
/// * `conn` - 数据库连接
/// * `sql` - SQL 计数查询语句
/// * `params` - 查询参数
pub fn count(conn: &Connection, sql: &str, params: &[&dyn ToSql]) -> Result<i64, String> {
    conn.query_row(sql, params, |row| row.get(0))
        .map_err(|e| e.to_string())
}
