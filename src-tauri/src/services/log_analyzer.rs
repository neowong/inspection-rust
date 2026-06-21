use serde::Serialize;

/// Structured log entry parsed from device syslog output.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub hostname: String,
    pub severity: String,
    pub module: String,
    pub mnemonic: String,
    pub message: String,
}

/// Result of parsing a batch of log output.
#[derive(Debug, Clone, Serialize)]
pub struct LogAnalysisResult {
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
    pub debug: usize,
    pub entries: Vec<LogEntry>,
    pub summary: String,
}

/// H3C/Huawei severity levels (digit after module name).
/// e.g. SSHS/6/... → 6 = informational
fn h3c_severity(level: u8) -> &'static str {
    match level {
        0..=1 => "EMERG",
        2 => "CRIT",
        3 => "ERROR",
        4 => "WARNING",
        5 => "NOTICE",
        6 => "INFO",
        7 => "DEBUG",
        _ => "INFO",
    }
}

/// Cisco severity levels (digit after hyphen).
/// e.g. %LINEPROTO-5-UPDOWN → 5 = notification
fn cisco_severity(level: u8) -> &'static str {
    match level {
        0 => "EMERG",
        1 => "ALERT",
        2 => "CRIT",
        3 => "ERROR",
        4 => "WARNING",
        5 => "NOTICE",
        6 => "INFO",
        7 => "DEBUG",
        _ => "INFO",
    }
}

fn classify_severity(sev: &str) -> &str {
    match sev {
        "EMERG" | "ALERT" | "CRIT" | "ERROR" => "ERROR",
        "WARNING" | "NOTICE" => "WARNING",
        "INFO" => "INFO",
        "DEBUG" => "DEBUG",
        _ => "INFO",
    }
}

/// Parse raw log output (e.g. from `display logbuffer`) into structured entries.
/// Supports H3C/Huawei and Cisco syslog formats.
pub fn parse_logs(raw: &str, vendor: &str) -> LogAnalysisResult {
    let vendor_lower = vendor.to_lowercase();
    let mut entries = Vec::new();
    let mut errors = 0usize;
    let mut warnings = 0usize;
    let mut info = 0usize;
    let mut debug = 0usize;

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let entry = if vendor_lower.contains("h3c") || vendor_lower.contains("huawei") || vendor_lower.contains("华三") || vendor_lower.contains("华为") {
            parse_h3c_line(line)
        } else if vendor_lower.contains("cisco") || vendor_lower.contains("思科") || vendor_lower.contains("ruijie") || vendor_lower.contains("锐捷") {
            parse_cisco_line(line)
        } else {
            None
        };

        if let Some(e) = entry {
            match classify_severity(&e.severity) {
                "ERROR" => errors += 1,
                "WARNING" => warnings += 1,
                "INFO" => info += 1,
                "DEBUG" => debug += 1,
                _ => info += 1,
            }
            entries.push(e);
        }
    }

    let total = entries.len();
    LogAnalysisResult {
        total,
        errors,
        warnings,
        info,
        debug,
        summary: format!("共 {} 条日志: ERROR={}, WARNING={}, INFO={}, DEBUG={}", total, errors, warnings, info, debug),
        entries,
    }
}

/// H3C/Huawei format: %Mon DD HH:MM:SS:mmm YYYY hostname MODULE/SEVERITY/MNEMONIC: message
fn parse_h3c_line(line: &str) -> Option<LogEntry> {
    let line = line.trim();
    if !line.starts_with('%') {
        return None;
    }

    // Split into: timestamp(5 tokens) + hostname(1) + rest(header + message)
    // splitn(6): 第 6 段为剩余全部，含 "MODULE/SEVERITY/MNEMONIC: message"
    let rest = &line[1..];
    let mut tokens = rest.splitn(6, ' ');
    let mon = tokens.next()?;
    let day = tokens.next()?;
    let time = tokens.next()?;
    let year = tokens.next()?;
    let hostname = tokens.next()?.to_string();
    let remaining = tokens.next()?;

    let timestamp = format!("{} {} {} {}", mon, day, time, year);

    // remaining: "MODULE/SEVERITY/MNEMONIC: message..."
    let colon_pos = remaining.find(':')?;
    let header = &remaining[..colon_pos];
    let message = remaining[colon_pos + 1..].trim().to_string();

    let header_parts: Vec<&str> = header.split('/').collect();
    if header_parts.len() < 3 {
        return None;
    }
    let module = header_parts[0].to_string();
    let severity_code: u8 = header_parts[1].parse().ok()?;
    let mnemonic = header_parts[2..].join("/");
    let severity = h3c_severity(severity_code).to_string();

    Some(LogEntry {
        timestamp,
        hostname,
        severity,
        module,
        mnemonic,
        message,
    })
}

/// Cisco format: *Mon DD HH:MM:SS.mmm: %FACILITY-SEVERITY-MNEMONIC: message
fn parse_cisco_line(line: &str) -> Option<LogEntry> {
    let line = line.trim();
    if !line.starts_with('*') && !line.starts_with('.') {
        return None;
    }

    let content = &line[1..];

    // Split at ": %" to separate timestamp from facility info
    let split_pos = content.find(": %")?;
    let ts_part = &content[..split_pos];
    let facility_part = &content[split_pos + 2..]; // skip ": "

    let ts_tokens: Vec<&str> = ts_part.splitn(4, ' ').collect();
    if ts_tokens.len() < 3 { return None; }
    let timestamp = format!("{} {} {}", ts_tokens[0], ts_tokens[1], ts_tokens[2]);

    // facility_part: "%FACILITY-SEVERITY-MNEMONIC: message..."
    let colon_pos = facility_part.find(':')?;
    let header = facility_part[..colon_pos].trim_start_matches('%');
    let message = facility_part[colon_pos + 1..].trim().to_string();

    let header_parts: Vec<&str> = header.split('-').collect();
    if header_parts.len() < 3 {
        return None;
    }
    let module = header_parts[0].to_string();
    let severity_code: u8 = header_parts[1].parse().ok()?;
    let mnemonic = header_parts[2..].join("-");
    let severity = cisco_severity(severity_code).to_string();

    Some(LogEntry {
        timestamp,
        hostname: String::new(),
        severity,
        module,
        mnemonic,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_h3c_log() {
        let line = "%May 30 11:03:59:450 2026 DeviceA SSHS/6/SSHS_VERSION_MISMATCH: SSH client 10.0.0.100 failed to log in because of version mismatch.";
        let entry = parse_h3c_line(line).unwrap();
        assert_eq!(entry.hostname, "DeviceA");
        assert_eq!(entry.severity, "INFO");
        assert_eq!(entry.module, "SSHS");
        assert_eq!(entry.mnemonic, "SSHS_VERSION_MISMATCH");
        assert!(!entry.message.is_empty());
        assert!(entry.message.contains("version mismatch"));
    }

    #[test]
    fn test_parse_h3c_error() {
        let line = "%May 30 11:03:59:450 2026 aHope SHELL/3/SHELL_CMD_ERR: command not found.";
        let entry = parse_h3c_line(line).unwrap();
        assert_eq!(entry.severity, "ERROR");
        assert_eq!(entry.module, "SHELL");
    }

    #[test]
    fn test_parse_cisco_log() {
        let line = "*May 30 11:03:59.450: %LINEPROTO-5-UPDOWN: Line protocol on Interface GigabitEthernet0/1, changed state to up";
        let entry = parse_cisco_line(line).unwrap();
        assert_eq!(entry.severity, "NOTICE");
        assert_eq!(entry.module, "LINEPROTO");
        assert_eq!(entry.mnemonic, "UPDOWN");
    }

    #[test]
    fn test_parse_logs_h3c() {
        let raw = "%May 30 11:01:00:000 2026 aHope SSHS/6/LOGIN: admin logged in\n%May 30 11:02:00:000 2026 aHope SHELL/3/ERR: command failed\nsome non-log line\n%May 30 11:03:00:000 2026 aHope SSHS/5/LOGOUT: admin logged out";
        let result = parse_logs(raw, "H3C");
        assert_eq!(result.total, 3);
        assert_eq!(result.errors, 1);
        assert_eq!(result.warnings, 1);
        assert_eq!(result.info, 1);
    }

    #[test]
    fn test_parse_logs_non_log_text() {
        let raw = "display logbuffer\nSome header info\n%May 30 11:01:00:000 2026 aHope SSHS/6/LOGIN: admin logged in";
        let result = parse_logs(raw, "H3C");
        assert_eq!(result.total, 1);
        assert_eq!(result.info, 1);
    }
}
