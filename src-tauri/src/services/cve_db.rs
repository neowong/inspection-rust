use futures::StreamExt;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;

/// CVE 本地数据库管理。
/// 数据来源：TridentStack bulk.jsonl（完全免费，无需 API Key）
///
/// 数据库文件位置：APP_DATA_DIR/data/cve/cve_cache.db
/// 表结构：
///   cve_cache (cve_id TEXT PK, summary TEXT, cvss REAL, severity TEXT,
///              exploit_available INT, fix_version TEXT, products TEXT)

// ============================================================================
// 路径
// ============================================================================

fn db_dir() -> PathBuf {
    crate::APP_DATA_DIR
        .get()
        .map(|p| p.join("data").join("cve"))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn db_path() -> PathBuf {
    db_dir().join("cve_cache.db")
}

// ============================================================================
// 数据库初始化
// ============================================================================

pub fn init_db() -> Result<(), String> {
    let dir = db_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建 CVE 目录失败: {}", e))?;

    let conn = rusqlite::Connection::open(db_path()).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cve_cache (
            cve_id        TEXT PRIMARY KEY,
            summary       TEXT NOT NULL DEFAULT '',
            cvss          REAL NOT NULL DEFAULT 0.0,
            severity      TEXT NOT NULL DEFAULT 'info',
            exploit_available INTEGER NOT NULL DEFAULT 0,
            fix_version   TEXT,
            products      TEXT NOT NULL DEFAULT '[]',
            updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_cve_products ON cve_cache(products);
        CREATE INDEX IF NOT EXISTS idx_cve_cvss ON cve_cache(cvss DESC);
        CREATE TABLE IF NOT EXISTS cve_meta (
            key   TEXT PRIMARY KEY,
            value TEXT
        );
        CREATE TABLE IF NOT EXISTS cpe_cache (
            product    TEXT NOT NULL,
            version    TEXT NOT NULL,
            result     TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (product, version)
        );"
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// 获取数据库基本信息
pub fn db_info() -> Result<serde_json::Value, String> {
    init_db()?;
    let conn = rusqlite::Connection::open(db_path()).map_err(|e| e.to_string())?;
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM cve_cache", [], |r| r.get(0))
        .unwrap_or(0);
    let last_cve: String = conn
        .query_row(
            "SELECT COALESCE(MAX(cve_id), '') FROM cve_cache",
            [],
            |r| r.get(0),
        )
        .unwrap_or_default();
    let db_size = std::fs::metadata(db_path()).map(|m| m.len()).unwrap_or(0);
    Ok(serde_json::json!({
        "count": count,
        "last_cve": last_cve,
        "db_size": db_size,
        "db_path": db_path().to_string_lossy(),
    }))
}

/// 下载并导入全部 CVE 数据（bulk.jsonl 流式 + 进度回调）
pub async fn download_cve_db<F: Fn(usize, usize) + Send + 'static>(
    progress_cb: F,
) -> Result<(), String> {
    init_db()?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| format!("HTTP 客户端创建失败: {}", e))?;

    // 用批量导入模式：先写入临时表，再合并
    let response = client
        .get("https://tridentstack.com/api/v1/cve/bulk.jsonl")
        .header("User-Agent", "HopeInspection/1.0")
        .send()
        .await
        .map_err(|e| format!("下载 CVE 数据库失败: {}", e))?;

    let status = response.status();
    if !status.is_success() {
        return Err(format!("CVE 数据库下载失败 (HTTP {})", status));
    }

    let stream = response.bytes_stream();

    // 打开数据库连接（设为 WAL 模式提升写入性能）
    let conn = rusqlite::Connection::open(db_path()).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=OFF; PRAGMA cache_size=-8000;")
        .map_err(|e| e.to_string())?;

    conn.execute_batch("DROP TABLE IF EXISTS cve_cache_temp;")
        .map_err(|e| e.to_string())?;
    conn.execute_batch(
        "CREATE TABLE cve_cache_temp (
            cve_id TEXT PRIMARY KEY, summary TEXT, cvss REAL,
            severity TEXT, exploit_available INT, fix_version TEXT, products TEXT
        );"
    ).map_err(|e| e.to_string())?;

    let mut buffer = Vec::new();
    let mut total_processed = 0usize;
    let mut total_errors = 0usize;
    let mut batch_rows: Vec<(String, String, f64, String, i64, Option<String>, String)> = Vec::new();
    const BATCH_SIZE: usize = 500;
    let progress = std::sync::Arc::new(std::sync::Mutex::new(progress_cb));

    // 按行读取流
    let mut chunks = stream;
    while let Some(chunk_result) = chunks.next().await {
        let chunk = chunk_result.map_err(|e| format!("读取数据流失败: {}", e))?;
        buffer.extend_from_slice(&chunk);

        // 按 \n 分割处理完整行
        loop {
            let newline_pos = buffer.iter().position(|&b| b == b'\n');
            match newline_pos {
                Some(pos) => {
                    let line: Vec<u8> = buffer.drain(..=pos).collect();
                    let line_str = String::from_utf8_lossy(&line[..line.len().saturating_sub(1)]);
                    let trimmed = line_str.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    match parse_cve_line(trimmed) {
                        Ok(Some(row)) => {
                            batch_rows.push(row);
                            total_processed += 1;
                            if batch_rows.len() >= BATCH_SIZE {
                                if let Err(e) = insert_batch(&conn, &batch_rows) {
                                    total_errors += batch_rows.len();
                                    tracing::warn!("CVE 批量插入失败: {}", e);
                                }
                                let count = total_processed;
                                let cb = progress.lock().unwrap();
                                cb(count, 0);
                                batch_rows.clear();
                            }
                        }
                        Ok(None) => {}
                        Err(_) => {
                            total_errors += 1;
                        }
                    }
                }
                None => break,
            }
        }
    }

    // 处理剩余的缓冲区数据
    if !buffer.is_empty() {
        let line_str = String::from_utf8_lossy(&buffer);
        if let Ok(Some(row)) = parse_cve_line(line_str.trim()) {
            batch_rows.push(row);
            total_processed += 1;
        }
    }

    // 插入最后一批
    if !batch_rows.is_empty() {
        if let Err(e) = insert_batch(&conn, &batch_rows) {
            tracing::warn!("CVE 最终批量插入失败: {}", e);
        }
    }

    // 从临时表合并到正式表（显式指定列，排除 cve_cache 的 updated_at 默认列）
    conn.execute_batch(
        "INSERT OR REPLACE INTO cve_cache (cve_id, summary, cvss, severity, exploit_available, fix_version, products)
         SELECT cve_id, summary, cvss, severity, exploit_available, fix_version, products FROM cve_cache_temp;
         DROP TABLE IF EXISTS cve_cache_temp;"
    ).map_err(|e| e.to_string())?;

    // 记录总数
    conn.execute(
        "INSERT OR REPLACE INTO cve_meta (key, value) VALUES ('total_cves', ?1)",
        rusqlite::params![total_processed.to_string()],
    ).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO cve_meta (key, value) VALUES ('last_updated', datetime('now'))",
        [],
    ).map_err(|e| e.to_string())?;

    let p = progress.lock().unwrap();
    p(total_processed, total_errors);

    Ok(())
}

// ============================================================================
// 批量插入
// ============================================================================

fn insert_batch(
    conn: &rusqlite::Connection,
    rows: &[(String, String, f64, String, i64, Option<String>, String)],
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(
            "INSERT OR IGNORE INTO cve_cache_temp
             (cve_id, summary, cvss, severity, exploit_available, fix_version, products)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .map_err(|e| e.to_string())?;

    for row in rows {
        stmt.execute(rusqlite::params![
            row.0, row.1, row.2, row.3, row.4, row.5, row.6
        ])
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ============================================================================
// 解析单行 JSONL
// ============================================================================

#[derive(Debug, Deserialize)]
struct BulkCveLine {
    #[serde(rename = "cveId")]
    cve_id: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    cvss: Option<serde_json::Value>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(rename = "isKev", default)]
    is_kev: Option<bool>,
    #[serde(default)]
    remediation: Option<BulkRemediation>,
}

#[derive(Debug, Deserialize)]
struct BulkRemediation {
    #[serde(default)]
    products: Option<Vec<BulkProduct>>,
}

#[derive(Debug, Deserialize)]
struct BulkProduct {
    #[serde(rename = "fixedVersion", default)]
    fixed_version: Option<String>,
    #[serde(default)]
    product: Option<String>,
}

fn parse_cve_line(line: &str) -> Result<Option<(String, String, f64, String, i64, Option<String>, String)>, String> {
    let parsed: BulkCveLine = serde_json::from_str(line)
        .map_err(|e| format!("JSON 解析失败: {}", e))?;

    let cve_id = match parsed.cve_id {
        Some(id) => id,
        None => return Ok(None),
    };

    let summary = parsed.description.unwrap_or_default();
    let cvss = match parsed.cvss {
        Some(v) => {
            if let Some(n) = v.as_f64() { n }
            else if let Some(obj) = v.as_object() {
                obj.get("baseScore").and_then(|s| s.as_f64()).unwrap_or(0.0)
            } else { 0.0 }
        }
        None => 0.0,
    };
    let severity = parsed.severity.unwrap_or(if cvss >= 9.0 { "CRITICAL".to_string() }
        else if cvss >= 7.0 { "HIGH".to_string() }
        else if cvss >= 4.0 { "MEDIUM".to_string() }
        else { "LOW".to_string() });
    let exploit = if parsed.is_kev.unwrap_or(false) { 1 } else { 0 };

    // 提取修复版本
    let fix_version = parsed.remediation.as_ref()
        .and_then(|r| r.products.as_ref())
        .and_then(|p| p.first())
        .and_then(|pp| pp.fixed_version.clone());

    // 提取产品名列表（用于本地查询匹配）
    let products = parsed.remediation.as_ref()
        .and_then(|r| r.products.as_ref())
        .map(|p| {
            p.iter()
                .filter_map(|pp| pp.product.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let products_json = serde_json::to_string(&products).unwrap_or_else(|_| "[]".to_string());

    Ok(Some((cve_id, summary, cvss, severity.to_lowercase(), exploit, fix_version, products_json)))
}

// ============================================================================
// 本地查询
// ============================================================================

/// 按产品名查询本地 CVE 数据库
pub fn query_cve_by_product_local(product: &str) -> Result<Vec<crate::services::cve_checker::CveItem>, String> {
    init_db()?;
    let conn = rusqlite::Connection::open(db_path()).map_err(|e| e.to_string())?;

    let product_lower = product.to_lowercase();
    let pattern = format!("%{}%", &product_lower);

    // 在多列中搜索，尽可能匹配
    let mut stmt = conn
        .prepare(
            "SELECT cve_id, summary, cvss, severity, exploit_available, fix_version
             FROM cve_cache
             WHERE LOWER(products) LIKE ?1
                OR LOWER(summary) LIKE ?1
                OR LOWER(cve_id) LIKE ?1
             ORDER BY cvss DESC LIMIT 50",
        )
        .map_err(|e| e.to_string())?;

    let results = stmt
        .query_map(rusqlite::params![pattern], |row| {
            Ok(crate::services::cve_checker::CveItem {
                cve_id: row.get(0)?,
                summary: row.get(1)?,
                cvss_score: row.get(2)?,
                severity: row.get(3)?,
                fix_version: row.get(5)?,
                exploit_available: row.get::<_, i64>(4)? != 0,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();

    Ok(results)
}

/// 从 CPE 缓存中查询指定产品版本的 CVE 结果
pub fn get_cpe_cache(product: &str, version: &str) -> Option<Vec<crate::services::cve_checker::CveItem>> {
    let path = db_path();
    if !path.exists() { return None; }
    let conn = rusqlite::Connection::open(&path).ok()?;
    let result: String = conn.query_row(
        "SELECT result FROM cpe_cache WHERE product = ?1 AND version = ?2",
        rusqlite::params![product.to_lowercase(), version],
        |r| r.get(0),
    ).ok()?;
    serde_json::from_str(&result).ok()
}

/// 将 CPE 查询结果存入缓存
pub fn set_cpe_cache(product: &str, version: &str, cves: &[crate::services::cve_checker::CveItem]) {
    let path = db_path();
    if !path.exists() { return; }
    if let Ok(conn) = rusqlite::Connection::open(&path) {
        let json = serde_json::to_string(cves).unwrap_or_default();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO cpe_cache (product, version, result, created_at) VALUES (?1, ?2, ?3, datetime('now'))",
            rusqlite::params![product.to_lowercase(), version, json],
        );
    }
}

/// 检查是否有本地 CVE 数据
pub fn has_local_data() -> bool {
    init_db().ok();
    let path = db_path();
    if !path.exists() {
        return false;
    }
    match rusqlite::Connection::open(&path) {
        Ok(conn) => {
            conn.query_row("SELECT COUNT(*) FROM cve_cache", [], |r| r.get::<_, i64>(0))
                .map(|c| c > 100)
                .unwrap_or(false)
        }
        Err(_) => false,
    }
}
