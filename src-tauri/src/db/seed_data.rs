use rusqlite::Connection;

pub fn seed_command_pool(conn: &Connection) -> Result<usize, String> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM command_pool", [], |r| r.get(0)).unwrap_or(0);
    if count > 0 {
        return Ok(0);
    }

    let commands = vec![
        ("H3C", "display version", "查看系统版本", "version"),
        ("H3C", "display clock", "查看系统时钟", "clock"),
        ("H3C", "display device", "查看设备信息", "hardware"),
        ("H3C", "display device manuinfo", "查看制造信息", "hardware"),
        ("H3C", "display cpu-usage", "查看 CPU 利用率", "cpu"),
        ("H3C", "display memory-usage", "查看内存利用率", "memory"),
        ("H3C", "display power", "查看电源状态", "power"),
        ("H3C", "display fan", "查看风扇状态", "fan"),
        ("H3C", "display environment", "查看环境状态", "env"),
        ("H3C", "display interface brief", "查看接口概要", "interface"),
        ("H3C", "display vlan", "查看 VLAN 信息", "vlan"),
        ("H3C", "display logbuffer", "查看日志摘要", "log"),
        ("H3C", "display ip routing-table", "查看路由表", "protocol"),
        ("H3C", "display ospf peer brief", "查看 OSPF 邻居", "protocol"),
        ("H3C", "display bgp peer brief", "查看 BGP 邻居", "protocol"),
        ("H3C", "display current-configuration | include sysname", "主机名", "general"),
        ("华为", "display version", "查看系统版本", "version"),
        ("华为", "display clock", "查看系统时钟", "clock"),
        ("华为", "display device", "查看设备信息", "hardware"),
        ("华为", "display cpu-usage", "查看 CPU 利用率", "cpu"),
        ("华为", "display memory-usage", "查看内存利用率", "memory"),
        ("华为", "display esn", "查看设备序列号", "hardware"),
        ("华为", "display interface brief", "查看接口概要", "interface"),
        ("华为", "display ip routing-table", "查看路由表", "protocol"),
        ("华为", "display vlan", "查看 VLAN 信息", "vlan"),
        ("华为", "display ospf peer brief", "查看 OSPF 邻居", "protocol"),
        ("思科", "show version", "查看系统版本", "version"),
        ("思科", "show clock", "查看系统时钟", "clock"),
        ("思科", "show inventory", "查看设备清单", "hardware"),
        ("思科", "show processes cpu", "查看 CPU 利用率", "cpu"),
        ("思科", "show processes memory", "查看内存利用率", "memory"),
        ("思科", "show environment", "查看环境状态", "env"),
        ("思科", "show interface status", "查看接口状态", "interface"),
        ("思科", "show vlan", "查看 VLAN 信息", "vlan"),
        ("思科", "show logging", "查看日志摘要", "log"),
        ("思科", "show ip route", "查看路由表", "protocol"),
        ("思科", "show ospf neighbor", "查看 OSPF 邻居", "protocol"),
        ("思科", "show bgp summary", "查看 BGP 摘要", "protocol"),
        ("思科", "show running-config | include hostname", "主机名", "general"),
        ("锐捷", "show version", "查看系统版本", "version"),
        ("锐捷", "show clock", "查看系统时钟", "clock"),
        ("锐捷", "show device", "查看设备信息", "hardware"),
        ("锐捷", "show cpu", "查看 CPU 利用率", "cpu"),
        ("锐捷", "show memory", "查看内存利用率", "memory"),
        ("锐捷", "show interface status", "查看接口状态", "interface"),
        ("锐捷", "show vlan", "查看 VLAN 信息", "vlan"),
        ("锐捷", "show log", "查看日志摘要", "log"),
        ("锐捷", "show ip route", "查看路由表", "protocol"),
        ("锐捷", "show running-config | include hostname", "主机名", "general"),
    ];

    let mut created = 0;
    for (vendor, cmd, desc, cat) in commands {
        conn.execute(
            "INSERT INTO command_pool (vendor, command, description, category) VALUES (?1,?2,?3,?4)",
            rusqlite::params![vendor, cmd, desc, cat],
        ).ok();
        created += 1;
    }

    tracing::info!("Seed data: {} commands inserted", created);
    Ok(created)
}
