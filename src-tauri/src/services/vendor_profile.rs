//! 厂商适配层 — 根据设备厂商返回对应的 SSH 执行策略
//!
//! 网络设备（H3C/华为/思科/飞塔等）使用交互式 Shell 会话，
//! Linux 服务器使用 exec channel（非交互）。

/// 执行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecMode {
    /// 交互式 Shell — 持久 PTY 会话，提示符检测
    Shell,
    /// Exec channel — 每条命令独立 channel，无需提示符检测
    Exec,
}

/// 判断厂商是否为 Linux 系统（OS 层面，非数据库/网络设备）
pub fn is_linux_vendor(vendor: &str) -> bool {
    matches!(vendor.to_lowercase().as_str(),
        "linux" | "ubuntu" | "centos" | "rocky" | "debian" | "rhel" | "suse" | "fedora" | "almalinux")
}

/// 判断厂商是否为数据库（需要注入认证信息、使用数据库客户端执行命令）
pub fn is_db_vendor(vendor: &str) -> bool {
    matches!(vendor.to_lowercase().as_str(),
        "mysql" | "mariadb" | "postgresql" | "postgres" | "mongodb" | "redis" |
        "oracle" | "sql" | "mssql" | "达梦")
}

/// 厂商行为配置
pub struct VendorProfile {
    pub exec_mode: ExecMode,
}

/// 根据厂商名称获取对应的 VendorProfile
pub fn get_profile(vendor: &str) -> VendorProfile {
    VendorProfile {
        exec_mode: if is_linux_vendor(vendor) { ExecMode::Exec } else { ExecMode::Shell },
    }
}
