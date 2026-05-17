use rusqlite::{Connection, types::ToSql};

/// Execute a query that maps rows to a type T using a callback.
/// Returns Vec<T> - avoids borrow checker issues with chained collect().
pub fn query_all<T, F>(conn: &Connection, sql: &str, params: &[&dyn ToSql], f: F) -> Result<Vec<T>, String>
where
    F: Fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows: Vec<T> = stmt.query_map(params, f)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// Query a single row, returns Option<T>
pub fn query_one<T, F>(conn: &Connection, sql: &str, params: &[&dyn ToSql], f: F) -> Result<Option<T>, String>
where
    F: Fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
{
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let mut rows: Vec<T> = stmt.query_map(params, f)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows.pop())
}

/// Get a count from SQL
pub fn count(conn: &Connection, sql: &str, params: &[&dyn ToSql]) -> Result<i64, String> {
    conn.query_row(sql, params, |r| r.get(0)).map_err(|e| e.to_string())
}
