//! 厂商适配层 — 根据设备厂商返回对应的 SSH 执行策略
//!
//! 网络设备（H3C/华为/思科等）使用交互式 Shell 会话，
//! Linux 服务器使用 exec channel（非交互）。

/// 执行模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecMode {
    /// 交互式 Shell — 持久 PTY 会话，提示符检测
    Shell,
    /// Exec channel — 每条命令独立 channel，无需提示符检测
    Exec,
}

/// sudo 提权模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SudoMode {
    /// 不需要提权
    None,
    /// 通过 stdin 写入 sudo 密码（sudo -S）
    PipePassword,
}

/// 厂商行为配置
pub struct VendorProfile {
    pub exec_mode: ExecMode,
    pub sudo_mode: SudoMode,
}

/// 根据厂商名称获取对应的 VendorProfile
///
/// 匹配规则：精确匹配 → 小写模糊匹配 → 默认 Shell
pub fn get_profile(vendor: &str) -> VendorProfile {
    let lower = vendor.to_lowercase();
    match lower.as_str() {
        "linux" | "ubuntu" | "centos" | "rocky" | "debian" | "rhel" | "suse" | "fedora" | "almalinux" => {
            VendorProfile {
                exec_mode: ExecMode::Exec,
                sudo_mode: SudoMode::PipePassword,
            }
        }
        _ => VendorProfile {
            exec_mode: ExecMode::Shell,
            sudo_mode: SudoMode::None,
        },
    }
}
