use rusqlite::Connection;

pub fn seed_command_pool(conn: &Connection) -> Result<usize, String> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM command_pool", [], |r| r.get(0)).unwrap_or(0);
    if count > 0 {
        return Ok(0);
    }

    let commands = vec![
        ("MySQL", "SHOW VARIABLES LIKE 'version%';", "查看 MySQL 版本信息", "版本", "db"),
        ("MySQL", "SHOW DATABASES;", "查看所有数据库", "数据库", "db"),
        ("MySQL", "SHOW PROCESSLIST;", "查看当前连接进程", "连接", "db"),
        ("MySQL", "SHOW ENGINE INNODB STATUS\\G", "InnoDB 引擎状态", "引擎", "db"),
        ("MySQL", "SELECT * FROM information_schema.TABLES WHERE TABLE_SCHEMA NOT IN ('information_schema','performance_schema','mysql','sys');", "查看业务表清单", "数据库", "db"),
        ("MySQL", "SELECT user, host, plugin FROM mysql.user;", "查看数据库用户", "权限", "db"),
        ("MySQL", "SHOW SLAVE STATUS\\G", "查看主从复制状态", "复制", "db"),
        ("MySQL", "SHOW MASTER STATUS;", "查看主库 Binlog 位点", "复制", "db"),
        ("PostgreSQL", "SELECT version();", "查看 PG 版本信息", "版本", "db"),
        ("PostgreSQL", "SELECT datname FROM pg_database;", "查看所有数据库", "数据库", "db"),
        ("PostgreSQL", "SELECT * FROM pg_stat_activity;", "查看当前连接会话", "连接", "db"),
        ("PostgreSQL", "SELECT schemaname, tablename, tableowner FROM pg_tables WHERE schemaname NOT IN ('pg_catalog','information_schema');", "查看业务表清单", "数据库", "db"),
        ("PostgreSQL", "SELECT * FROM pg_stat_user_tables;", "查看用户表统计信息", "性能", "db"),
        ("PostgreSQL", "SELECT * FROM pg_stat_database;", "查看数据库级统计", "性能", "db"),
        ("PostgreSQL", "SELECT * FROM pg_replication_slots;", "查看复制槽状态", "复制", "db"),
        ("Oracle", "SELECT * FROM v$version;", "查看 Oracle 版本信息", "版本", "db"),
        ("Oracle", "SELECT name FROM v$database;", "查看数据库名", "数据库", "db"),
        ("Oracle", "SELECT username FROM dba_users ORDER BY username;", "查看所有数据库用户", "权限", "db"),
        ("Oracle", "SELECT sid, serial#, username, status FROM v$session;", "查看当前会话", "连接", "db"),
        ("Oracle", "SELECT tablespace_name, status FROM dba_tablespaces;", "查看表空间状态", "存储", "db"),
        ("H3C", "display version", "查看系统版本", "version", "ssh"),
        ("H3C", "display clock", "查看系统时钟", "clock", "ssh"),
        ("H3C", "display device", "查看设备信息", "hardware", "ssh"),
        ("H3C", "display device manuinfo", "查看制造信息", "hardware", "ssh"),
        ("H3C", "display cpu-usage", "查看 CPU 利用率", "cpu", "ssh"),
        ("H3C", "display memory-usage", "查看内存利用率", "memory", "ssh"),
        ("H3C", "display power", "查看电源状态", "power", "ssh"),
        ("H3C", "display fan", "查看风扇状态", "fan", "ssh"),
        ("H3C", "display environment", "查看环境状态", "env", "ssh"),
        ("H3C", "display interface brief", "查看接口概要", "interface", "ssh"),
        ("H3C", "display vlan", "查看 VLAN 信息", "vlan", "ssh"),
        ("H3C", "display logbuffer", "查看日志摘要", "log", "ssh"),
        ("H3C", "display ip routing-table", "查看路由表", "protocol", "ssh"),
        ("H3C", "display current-configuration | include sysname", "主机名", "general", "ssh"),
        ("华为", "display version", "查看系统版本", "version", "ssh"),
        ("华为", "display clock", "查看系统时钟", "clock", "ssh"),
        ("华为", "display device", "查看设备信息", "hardware", "ssh"),
        ("华为", "display cpu-usage", "查看 CPU 利用率", "cpu", "ssh"),
        ("华为", "display memory-usage", "查看内存利用率", "memory", "ssh"),
        ("华为", "display esn", "查看设备序列号", "hardware", "ssh"),
        ("华为", "display interface brief", "查看接口概要", "interface", "ssh"),
        ("华为", "display ip routing-table", "查看路由表", "protocol", "ssh"),
        ("华为", "display vlan", "查看 VLAN 信息", "vlan", "ssh"),
        ("思科", "show version", "查看系统版本", "version", "ssh"),
        ("思科", "show clock", "查看系统时钟", "clock", "ssh"),
        ("思科", "show inventory", "查看设备清单", "hardware", "ssh"),
        ("思科", "show processes cpu", "查看 CPU 利用率", "cpu", "ssh"),
        ("思科", "show processes memory", "查看内存利用率", "memory", "ssh"),
        ("思科", "show environment", "查看环境状态", "env", "ssh"),
        ("思科", "show interface status", "查看接口状态", "interface", "ssh"),
        ("思科", "show vlan", "查看 VLAN 信息", "vlan", "ssh"),
        ("思科", "show logging", "查看日志摘要", "log", "ssh"),
        ("思科", "show ip route", "查看路由表", "protocol", "ssh"),
        ("思科", "show running-config | include hostname", "主机名", "general", "ssh"),
        ("Linux", "hostname", "查看主机名", "general", "ssh"),
        ("Linux", "cat /etc/os-release", "查看系统版本", "version", "ssh"),
        ("Linux", "uname -a", "查看内核信息", "version", "ssh"),
        ("Linux", "uptime", "查看系统运行时间和负载", "cpu", "ssh"),
        ("Linux", "free -h", "查看内存使用", "memory", "ssh"),
        ("Linux", "df -h", "查看磁盘使用", "disk", "ssh"),
        ("Linux", "lscpu", "查看 CPU 信息", "cpu", "ssh"),
        ("Linux", "lsblk", "查看块设备", "disk", "ssh"),
        ("Linux", "ip addr", "查看网络接口", "interface", "ssh"),
        ("Linux", "ss -tlnp", "查看监听端口", "protocol", "ssh"),
        ("Linux", "date", "查看系统时间", "clock", "ssh"),
    ];

    let mut created = 0;
    for (vendor, cmd, desc, cat, ctype) in commands {
        conn.execute(
            "INSERT INTO command_pool (vendor, command, description, category, command_type) VALUES (?1,?2,?3,?4,?5)",
            rusqlite::params![vendor, cmd, desc, cat, ctype],
        ).ok();
        created += 1;
    }

    tracing::info!("Seed data: {} commands inserted", created);
    Ok(created)
}
