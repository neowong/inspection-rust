use rusqlite::{params, Connection};

/// 种子命令数据（89条，H3C 24 + 华为 25 + 思科 22 + 锐捷 18）
/// 插入种子命令数据
///
/// 仅当 `command_pool` 表为空时执行插入。
/// 返回插入的行数，若表非空则返回 `Ok(0)`。
pub fn seed_command_pool(conn: &mut Connection) -> Result<usize, String> {
    // 检查命令池是否为空
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM command_pool", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    if count > 0 {
        return Ok(0);
    }

    let commands: Vec<(&str, &str, &str, &str)> = vec![
        // ==================== H3C (Comware V5/V7，已真机验证) ====================
        ("H3C", "display clock", "查看系统时钟", "clock"),
        ("H3C", "display ntp status", "查看 NTP 状态", "clock"),
        ("H3C", "display cpu-usage summary", "查看 CPU 利用率", "cpu"),
        ("H3C", "display environment", "查看环境状态", "env"),
        ("H3C", "display fan", "查看风扇状态", "fan"),
        ("H3C", "display current-configuration | include sysname", "主机名", "general"),
        ("H3C", "dir", "Flash空间使用情况", "hardware"),
        ("H3C", "display device", "查看设备信息", "hardware"),
        ("H3C", "display device manuinfo", "查看设备序列号", "hardware"),
        ("H3C", "display irf", "查看 IRF 状态", "hardware"),
        ("H3C", "display transceiver", "查看光模块信息", "hardware"),
        ("H3C", "display interface", "查看接口详细信息", "interface"),
        ("H3C", "display interface brief", "查看接口概要", "interface"),
        ("H3C", "display logbuffer last-mins 5", "查看日志摘要", "log"),
        ("H3C", "display memory summary", "查看内存利用率", "memory"),
        ("H3C", "display power", "查看电源状态", "power"),
        ("H3C", "display arp", "查看 ARP 表", "protocol"),
        ("H3C", "display ip routing-table", "查看路由表", "protocol"),
        ("H3C", "display stp brief", "生成树状态", "protocol"),
        ("H3C", "display version", "查看系统版本", "version"),
        ("H3C", "display mac-address", "查看 MAC 地址表", "vlan"),
        ("H3C", "display vlan", "查看 VLAN 信息", "vlan"),
        ("H3C", "display wlan ap all", "检查 AP 整体上线状态", "wireless"),
        ("H3C", "display wlan service-template", "无线服务模板（SSID）状态", "wireless"),
        // ==================== 华为 (Huawei VRP) ====================
        ("华为", "display clock", "查看系统时钟", "clock"),
        ("华为", "display ntp-status", "查看 NTP 状态", "clock"),
        ("华为", "display cpu-usage", "查看 CPU 利用率", "cpu"),
        ("华为", "display environment", "查看环境状态", "env"),
        ("华为", "display temperature all", "查看所有温度传感器", "env"),
        ("华为", "display fan", "查看风扇状态", "fan"),
        ("华为", "display current-configuration | include sysname", "主机名", "general"),
        ("华为", "display device", "查看设备信息", "hardware"),
        ("华为", "display elabel", "查看设备序列号/电子标签", "hardware"),
        ("华为", "display health", "查看设备健康状态", "hardware"),
        ("华为", "display stack", "查看堆叠状态", "hardware"),
        ("华为", "display transceiver", "查看光模块信息", "hardware"),
        ("华为", "display interface", "查看接口详细信息", "interface"),
        ("华为", "display interface brief", "查看接口概要", "interface"),
        ("华为", "display ip interface brief", "查看 IP 接口概要", "interface"),
        ("华为", "display logbuffer", "查看日志摘要", "log"),
        ("华为", "display memory-usage", "查看内存利用率", "memory"),
        ("华为", "display power", "查看电源状态", "power"),
        ("华为", "display arp", "查看 ARP 表", "protocol"),
        ("华为", "display ip routing-table", "查看路由表", "protocol"),
        ("华为", "display stp brief", "生成树状态", "protocol"),
        ("华为", "display version", "查看系统版本", "version"),
        ("华为", "display mac-address", "查看 MAC 地址表", "vlan"),
        ("华为", "display vlan", "查看 VLAN 信息", "vlan"),
        // ==================== 思科 (Cisco IOS) ====================
        ("思科", "show clock", "查看系统时钟", "clock"),
        ("思科", "show ntp associations", "查看 NTP 状态", "clock"),
        ("思科", "show processes cpu", "查看 CPU 利用率", "cpu"),
        ("思科", "show processes cpu history", "查看 CPU 历史使用", "cpu"),
        ("思科", "show environment", "查看环境状态", "env"),
        ("思科", "show running-config", "查看运行配置", "general"),
        ("思科", "show running-config | include hostname", "主机名", "general"),
        ("思科", "show version | include uptime", "查看运行时间", "general"),
        ("思科", "show inventory", "查看设备信息", "hardware"),
        ("思科", "show interface summary", "查看接口摘要", "interface"),
        ("思科", "show interfaces status", "查看接口状态", "interface"),
        ("思科", "show interfaces trunk", "查看 Trunk", "interface"),
        ("思科", "show ip interface brief", "查看 IP 接口概要", "interface"),
        ("思科", "show logging", "查看日志", "log"),
        ("思科", "show memory statistics", "查看内存利用率", "memory"),
        ("思科", "show power", "查看电源状态", "power"),
        ("思科", "show arp", "查看 ARP 表", "protocol"),
        ("思科", "show cdp neighbors", "查看 CDP 邻居", "protocol"),
        ("思科", "show ip route", "查看路由表", "protocol"),
        ("思科", "show spanning-tree", "生成树状态", "protocol"),
        ("思科", "show version", "查看系统版本", "version"),
        ("思科", "show mac address-table", "查看 MAC 地址表", "vlan"),
        ("思科", "show vlan brief", "查看 VLAN 信息", "vlan"),
        // ==================== 锐捷 (Ruijie RGOS) ====================
        ("锐捷", "show clock", "查看系统时钟", "clock"),
        ("锐捷", "show ntp", "查看 NTP 状态", "clock"),
        ("锐捷", "show cpu", "查看 CPU 利用率", "cpu"),
        ("锐捷", "show environment", "查看环境状态", "env"),
        ("锐捷", "show temperature", "查看设备温度", "env"),
        ("锐捷", "show running-config | include hostname", "主机名", "general"),
        ("锐捷", "show inventory", "查看设备信息/序列号", "hardware"),
        ("锐捷", "show transceiver", "查看光模块信息", "hardware"),
        ("锐捷", "show interface", "查看接口详细信息", "interface"),
        ("锐捷", "show interface brief", "查看接口概要", "interface"),
        ("锐捷", "show interfaces status", "查看接口状态", "interface"),
        ("锐捷", "show logging", "查看日志", "log"),
        ("锐捷", "show memory", "查看内存利用率", "memory"),
        ("锐捷", "show power", "查看电源状态", "power"),
        ("锐捷", "show arp", "查看 ARP 表", "protocol"),
        ("锐捷", "show dhcp snooping", "查看 DHCP Snooping", "protocol"),
        ("锐捷", "show ip route", "查看路由表", "protocol"),
        ("锐捷", "show spanning-tree", "生成树状态", "protocol"),
        ("锐捷", "show version", "查看系统版本", "version"),
        ("锐捷", "show mac-address-table", "查看 MAC 地址表", "vlan"),
        ("锐捷", "show vlan", "查看 VLAN 信息", "vlan"),
    ];

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    {
        let mut stmt = tx
            .prepare("INSERT INTO command_pool (vendor, command, description, category) VALUES (?1, ?2, ?3, ?4)")
            .map_err(|e| e.to_string())?;
        for (vendor, command, description, category) in &commands {
            stmt.execute(params![vendor, command, description, category])
                .map_err(|e| format!("插入种子命令失败 ({}): {}", command, e))?;
        }
    }
    tx.commit().map_err(|e| e.to_string())?;

    Ok(commands.len())
}
