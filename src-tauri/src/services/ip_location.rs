//! IP 归属地查询（离线 ip2region xdb）
//!
//! ip2region.xdb 作为外挂文件放在二进制同目录（或 resource 目录），
//! 启动时加载到内存。裸二进制分发需把 ip2region.xdb 放到 exe 同目录。
//! 查询时零拷贝返回 `国家|区域|省份|城市|ISP`，xdb-parse 微秒级。

use std::path::Path;

/// 加载 xdb 文件到内存
pub fn load_xdb(path: &Path) -> Result<Vec<u8>, String> {
    xdb_parse::load_file(path.to_path_buf()).map_err(|e| format!("加载 ip2region.xdb 失败: {}", e))
}

/// 查询 IP 归属地，返回原始 `国家|区域|省份|城市|ISP` 字符串（0 表示无值）
/// 失败（无效 IP/库中无记录）返回 None
pub fn lookup(db: &[u8], ip: &str) -> Option<String> {
    xdb_parse::search_ip(ip, db).ok().map(|s| s.to_string())
}

/// 检查 IP 是否为私有/局域网地址
pub fn is_private_ip(ip: &str) -> bool {
    let parts: Vec<u8> = match ip.split('.').map(|s| s.parse::<u8>().ok()).collect::<Option<Vec<_>>>() {
        Some(p) if p.len() == 4 => p,
        _ => return false,
    };
    // 10.0.0.0/8
    if parts[0] == 10 { return true; }
    // 172.16.0.0/12
    if parts[0] == 172 && (parts[1] & 0xF0) == 16 { return true; }
    // 192.168.0.0/16
    if parts[0] == 192 && parts[1] == 168 { return true; }
    // 127.0.0.0/8
    if parts[0] == 127 { return true; }
    false
}

/// 判断字段值是否为空（ip2region 用 "0" / "Reserved" / "保留" / "内网IP" 等表示无值）
fn is_field_empty(s: &str) -> bool {
    s.is_empty()
        || s == "0"
        || s.eq_ignore_ascii_case("Reserved")
        || s.eq_ignore_ascii_case("保留")
        || s == "内网IP"
        || s == "内网IP地址"
}

/// 将原始 `国家|区域|省份|城市|ISP` 格式化为可读字符串
/// 例：`中国|0|浙江省|杭州市|电信` → `中国 浙江省杭州市 电信`
///     `0|0|0|0|0` + 私有IP → `局域网`
///     `0|0|0|0|0` + 公网IP → 空串（无记录）
///     `Reserved|0|0|0|0` + 私有IP → `局域网`
pub fn format_region(raw: &str, ip: Option<&str>) -> String {
    let parts: Vec<&str> = raw.split('|').collect();
    if parts.len() < 5 {
        return raw.to_string();
    }
    let country = parts[0].trim();
    let region = parts[1].trim();
    let province = parts[2].trim();
    let city = parts[3].trim();
    let isp = parts[4].trim();

    // 全部为空值 → 无记录
    let all_empty = [country, region, province, city, isp]
        .iter()
        .all(|s| is_field_empty(s));
    if all_empty {
        // 私有/局域网地址返回"局域网"
        if let Some(addr) = ip {
            if is_private_ip(addr) {
                return "局域网".to_string();
            }
        }
        return String::new();
    }

    let mut bits: Vec<String> = Vec::new();
    // 国家
    if !is_field_empty(country) {
        bits.push(country.to_string());
    }
    // 省+市合并（省=市时去重）
    if !is_field_empty(province) {
        if !is_field_empty(city) && province != city {
            bits.push(format!("{}{}", province, city));
        } else {
            bits.push(province.to_string());
        }
    } else if !is_field_empty(city) {
        bits.push(city.to_string());
    }
    // 区域（如 "华东"）
    if !is_field_empty(region) {
        bits.push(region.to_string());
    }
    // ISP
    if !is_field_empty(isp) {
        bits.push(isp.to_string());
    }
    bits.join(" ")
}
