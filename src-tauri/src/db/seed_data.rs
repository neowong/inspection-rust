use rusqlite::{params, Connection};

/// 种子命令数据（含 H3C/华为/思科/锐捷/飞塔/Linux 常用巡检命令）
///
/// 使用 `INSERT OR IGNORE` 幂等插入，但会跳过被用户主动删除的命令（通过 `deleted_seed_commands` 墓碑表）。
/// 每次启动都会执行，确保种子数据完整且用户删除的命令不会复活。
pub fn seed_command_pool(conn: &mut Connection) -> Result<usize, String> {
    // 确保墓碑表存在
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS deleted_seed_commands (
            vendor TEXT NOT NULL,
            command TEXT NOT NULL,
            deleted_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(vendor, command)
        )"
    ).map_err(|e| e.to_string())?;
    let commands: Vec<(&str, &str, &str, &str, i64)> = vec![
        // ==================== H3C (Comware V5/V7，已真机验证) ====================
        ("H3C", "display clock", "查看系统时钟", "clock", 0),
        ("H3C", "display ntp status", "查看 NTP 状态", "clock", 0),
        ("H3C", "display cpu-usage summary", "查看 CPU 利用率", "performance", 0),
        ("H3C", "display environment", "查看环境状态", "env", 0),
        ("H3C", "display fan", "查看风扇状态", "hardware", 0),
        ("H3C", "display current-configuration | include sysname", "主机名", "general", 0),
        ("H3C", "dir", "Flash空间使用情况", "hardware", 0),
        ("H3C", "display device", "查看设备信息", "hardware", 0),
        ("H3C", "display device manuinfo", "查看设备序列号", "hardware", 0),
        ("H3C", "display irf", "查看 IRF 状态", "hardware", 0),
        ("H3C", "display transceiver", "查看光模块信息", "hardware", 0),
        ("H3C", "display interface", "查看接口详细信息", "interface", 0),
        ("H3C", "display interface brief", "查看接口概要", "interface", 0),
        ("H3C", "display logbuffer last-mins 5", "查看日志摘要", "log", 0),
        ("H3C", "display memory summary", "查看内存利用率", "performance", 0),
        ("H3C", "display power", "查看电源状态", "hardware", 0),
        ("H3C", "display arp", "查看 ARP 表", "protocol", 0),
        ("H3C", "display ip routing-table", "查看路由表", "protocol", 0),
        ("H3C", "display stp brief", "生成树状态", "protocol", 0),
        ("H3C", "display version", "查看系统版本", "version", 0),
        ("H3C", "display mac-address", "查看 MAC 地址表", "interface", 0),
        ("H3C", "display vlan", "查看 VLAN 信息", "interface", 0),
        ("H3C", "display wlan ap all", "检查 AP 整体上线状态", "wireless", 0),
        ("H3C", "display wlan service-template", "无线服务模板（SSID）状态", "wireless", 0),
        // ==================== 华为 (Huawei VRP) ====================
        ("华为", "display clock", "查看系统时钟", "clock", 0),
        ("华为", "display ntp-status", "查看 NTP 状态", "clock", 0),
        ("华为", "display cpu-usage", "查看 CPU 利用率", "performance", 0),
        ("华为", "display environment", "查看环境状态", "env", 0),
        ("华为", "display temperature all", "查看所有温度传感器", "env", 0),
        ("华为", "display fan", "查看风扇状态", "hardware", 0),
        ("华为", "display current-configuration | include sysname", "主机名", "general", 0),
        ("华为", "display device", "查看设备信息", "hardware", 0),
        ("华为", "display elabel", "查看设备序列号/电子标签", "hardware", 0),
        ("华为", "display health", "查看设备健康状态", "hardware", 0),
        ("华为", "display stack", "查看堆叠状态", "hardware", 0),
        ("华为", "display transceiver", "查看光模块信息", "hardware", 0),
        ("华为", "display interface", "查看接口详细信息", "interface", 0),
        ("华为", "display interface brief", "查看接口概要", "interface", 0),
        ("华为", "display ip interface brief", "查看 IP 接口概要", "interface", 0),
        ("华为", "display logbuffer", "查看日志摘要", "log", 0),
        ("华为", "display memory-usage", "查看内存利用率", "performance", 0),
        ("华为", "display power", "查看电源状态", "hardware", 0),
        ("华为", "display arp", "查看 ARP 表", "protocol", 0),
        ("华为", "display ip routing-table", "查看路由表", "protocol", 0),
        ("华为", "display stp brief", "生成树状态", "protocol", 0),
        ("华为", "display version", "查看系统版本", "version", 0),
        ("华为", "display mac-address", "查看 MAC 地址表", "interface", 0),
        ("华为", "display vlan", "查看 VLAN 信息", "interface", 0),
        // ==================== 思科 (Cisco IOS) ====================
        ("思科", "show clock", "查看系统时钟", "clock", 0),
        ("思科", "show ntp associations", "查看 NTP 状态", "clock", 0),
        ("思科", "show processes cpu", "查看 CPU 利用率", "performance", 0),
        ("思科", "show processes cpu history", "查看 CPU 历史使用", "performance", 0),
        ("思科", "show environment", "查看环境状态", "env", 0),
        ("思科", "show running-config", "查看运行配置", "general", 0),
        ("思科", "show running-config | include hostname", "主机名", "general", 0),
        ("思科", "show version | include uptime", "查看运行时间", "general", 0),
        ("思科", "show inventory", "查看设备信息", "hardware", 0),
        ("思科", "show interface summary", "查看接口摘要", "interface", 0),
        ("思科", "show interfaces status", "查看接口状态", "interface", 0),
        ("思科", "show interfaces trunk", "查看 Trunk", "interface", 0),
        ("思科", "show ip interface brief", "查看 IP 接口概要", "interface", 0),
        ("思科", "show logging", "查看日志", "log", 0),
        ("思科", "show memory statistics", "查看内存利用率", "performance", 0),
        ("思科", "show power", "查看电源状态", "hardware", 0),
        ("思科", "show arp", "查看 ARP 表", "protocol", 0),
        ("思科", "show cdp neighbors", "查看 CDP 邻居", "protocol", 0),
        ("思科", "show ip route", "查看路由表", "protocol", 0),
        ("思科", "show spanning-tree", "生成树状态", "protocol", 0),
        ("思科", "show version", "查看系统版本", "version", 0),
        ("思科", "show mac address-table", "查看 MAC 地址表", "interface", 0),
        ("思科", "show vlan brief", "查看 VLAN 信息", "interface", 0),
        // ==================== 锐捷 (Ruijie RGOS) ====================
        ("锐捷", "show clock", "查看系统时钟", "clock", 0),
        ("锐捷", "show ntp", "查看 NTP 状态", "clock", 0),
        ("锐捷", "show cpu", "查看 CPU 利用率", "performance", 0),
        ("锐捷", "show environment", "查看环境状态", "env", 0),
        ("锐捷", "show temperature", "查看设备温度", "env", 0),
        ("锐捷", "show running-config | include hostname", "主机名", "general", 0),
        ("锐捷", "show inventory", "查看设备信息/序列号", "hardware", 0),
        ("锐捷", "show transceiver", "查看光模块信息", "hardware", 0),
        ("锐捷", "show interface", "查看接口详细信息", "interface", 0),
        ("锐捷", "show interface brief", "查看接口概要", "interface", 0),
        ("锐捷", "show interfaces status", "查看接口状态", "interface", 0),
        ("锐捷", "show logging", "查看日志", "log", 0),
        ("锐捷", "show memory", "查看内存利用率", "performance", 0),
        ("锐捷", "show power", "查看电源状态", "hardware", 0),
        ("锐捷", "show arp", "查看 ARP 表", "protocol", 0),
        ("锐捷", "show dhcp snooping", "查看 DHCP Snooping", "protocol", 0),
        ("锐捷", "show ip route", "查看路由表", "protocol", 0),
        ("锐捷", "show spanning-tree", "生成树状态", "protocol", 0),
        ("锐捷", "show version", "查看系统版本", "version", 0),
        ("锐捷", "show mac-address-table", "查看 MAC 地址表", "interface", 0),
        ("锐捷", "show vlan", "查看 VLAN 信息", "interface", 0),
        // ==================== 飞塔 (Fortinet FortiGate) ====================
        ("飞塔", "get system status", "系统状态/主机名/型号/序列号", "version", 0),
        ("飞塔", "get system performance status", "查看性能状态", "performance", 0),
        ("飞塔", "diagnose sys top-summary", "查看进程与资源摘要", "performance", 0),
        ("飞塔", "get hardware status", "查看硬件状态", "hardware", 0),
        ("飞塔", "diagnose hardware sysinfo memory", "查看内存信息", "performance", 0),
        ("飞塔", "diagnose hardware sysinfo shm", "查看共享内存信息", "performance", 0),
        ("飞塔", "get system interface physical", "查看物理接口", "interface", 0),
        ("飞塔", "diagnose hardware deviceinfo nic", "查看网卡硬件信息", "interface", 0),
        ("飞塔", "get router info routing-table all", "查看路由表", "protocol", 0),
        ("飞塔", "diagnose ip arp list", "查看 ARP 表", "protocol", 0),
        ("飞塔", "diagnose sys session stat", "查看会话统计", "protocol", 0),
        ("飞塔", "get firewall policy", "查看防火墙策略", "security", 0),
        ("飞塔", "diagnose firewall iprope show", "查看策略匹配结构", "security", 0),
        ("飞塔", "get vpn ipsec tunnel summary", "查看 IPsec VPN 摘要", "vpn", 0),
        ("飞塔", "diagnose vpn tunnel list", "查看 VPN 隧道详情", "vpn", 0),
        ("飞塔", "get system ha status", "查看 HA 状态", "ha", 0),
        ("飞塔", "execute log display", "查看系统日志", "log", 0),
        ("飞塔", "diagnose debug crashlog read", "查看崩溃日志", "log", 0),
        // ==================== Linux ====================
        // 系统信息
        ("Linux", "hostnamectl", "主机名和系统信息", "system", 0),
        ("Linux", "uname -a", "内核版本", "system", 0),
        ("Linux", "cat /etc/os-release", "发行版信息", "system", 0),
        ("Linux", "uptime", "运行时间和负载", "system", 0),
        ("Linux", "timedatectl", "时区和时间同步", "system", 0),
        // CPU
        ("Linux", "lscpu", "CPU 架构信息", "performance", 0),
        ("Linux", "cat /proc/cpuinfo", "CPU 详细信息", "performance", 0),
        ("Linux", "top -bn1 | head -20", "CPU 使用率快照", "performance", 0),
        // 内存
        ("Linux", "free -h", "内存使用概况", "performance", 0),
        ("Linux", "cat /proc/meminfo", "内存详细信息", "performance", 0),
        // 磁盘
        ("Linux", "df -h", "磁盘使用率", "disk", 0),
        ("Linux", "lsblk", "块设备列表", "disk", 0),
        ("Linux", "iostat -x 1 1", "磁盘 I/O 统计", "disk", 0),
        ("Linux", "fdisk -l", "磁盘分区详情", "disk", 1),
        // 网络
        ("Linux", "ip addr", "网络接口和 IP", "network", 0),
        ("Linux", "ip route", "路由表", "network", 0),
        ("Linux", "ss -tlnp", "监听端口", "network", 0),
        ("Linux", "ss -s", "连接统计", "network", 0),
        ("Linux", "cat /etc/resolv.conf", "DNS 配置", "network", 0),
        // 服务
        ("Linux", "systemctl list-units --type=service --state=running --no-pager", "运行中的服务", "service", 0),
        ("Linux", "systemctl list-units --state=failed --no-pager", "失败的服务", "service", 0),
        // 进程
        ("Linux", "ps aux --sort=-%cpu | head -15", "CPU 占用 Top 进程", "process", 0),
        ("Linux", "ps aux --sort=-%mem | head -15", "内存占用 Top 进程", "process", 0),
        // 日志
        ("Linux", "journalctl -p err --no-pager -n 30", "最近错误日志", "log", 0),
        ("Linux", "dmesg | tail -30", "内核日志", "log", 0),
        ("Linux", "cat /var/log/syslog | tail -30", "系统日志", "log", 1),
        // 安全
        ("Linux", "last -10", "最近登录记录", "security", 0),
        ("Linux", "lastlog | grep -v Never", "所有用户最后登录", "security", 0),
        ("Linux", "cat /etc/passwd | grep -v nologin | grep -v false", "可登录用户", "security", 0),
        ("Linux", "iptables -L -n", "防火墙规则", "security", 1),
        // 硬件/内核
        ("Linux", "dmidecode -t system", "系统硬件信息", "hardware", 1),
        ("Linux", "lspci", "PCI 设备列表", "hardware", 0),
        ("Linux", "cat /proc/loadavg", "负载均值", "hardware", 0),
        ("Linux", "sysctl -a 2>/dev/null | head -30", "内核参数", "hardware", 0),
        // VM 环境检测
        ("Linux", "systemd-detect-virt", "虚拟化平台检测", "system", 0),
        ("Linux", "cat /sys/class/dmi/id/product_name", "虚拟化产品名称", "system", 0),
        ("Linux", "cat /sys/class/dmi/id/sys_vendor", "虚拟化厂商", "system", 0),
        ("Linux", "lscpu | grep -i 'hypervisor\\|virtualization'", "CPU 虚拟化特性", "performance", 0),
        ("Linux", "cat /proc/cpuinfo | grep 'model name' | head -1", "CPU 型号（含超线程）", "performance", 0),
        ("Linux", "dmidecode -t memory 2>/dev/null | head -10", "内存硬件信息", "memory", 1),
        ("Linux", "sudo dmidecode -t memory 2>/dev/null | grep -i Size", "物理内存大小", "memory", 1),
        ("Linux", "cat /sys/devices/system/clocksource/clocksource0/current_clocksource", "时钟源", "system", 0),
        // 定时任务
        ("Linux", "crontab -l", "当前用户定时任务", "schedule", 0),
        ("Linux", "systemctl list-timers --no-pager", "systemd 定时器", "schedule", 0),
        // ==================== Linux (CentOS 特有) ====================
        // --- 发行版特有命令 ---
        ("Linux", "cat /etc/centos-release", "CentOS 版本", "system", 0),
        ("Linux", "rpm -qa | head -20", "已安装 RPM 包", "system", 0),
        ("Linux", "yum repolist 2>/dev/null", "YUM 源配置", "system", 0),
        // ==================== Linux (Rocky 特有) ====================
        // --- 发行版特有命令 ---
        ("Linux", "cat /etc/rocky-release", "Rocky 版本", "system", 0),
        ("Linux", "rpm -qa | head -20", "已安装 RPM 包", "system", 0),
        ("Linux", "dnf repolist 2>/dev/null", "DNF 源配置", "system", 0),
        // ==================== Linux (Debian 特有) ====================
        // --- 发行版特有命令 ---
        ("Linux", "cat /etc/debian_version", "Debian 版本", "system", 0),
        ("Linux", "dpkg -l | head -20", "已安装 DEB 包", "system", 0),
        ("Linux", "cat /etc/apt/sources.list", "APT 源配置", "system", 0),
        // ==================== MySQL ====================
        ("MySQL", "mysql --version", "MySQL 版本", "version", 0),
        ("MySQL", "mysql -e 'SHOW VARIABLES LIKE \"version%\"'", "MySQL 详细版本信息", "version", 0),
        ("MySQL", "mysql -e 'SHOW STATUS' | head -30", "MySQL 运行状态摘要", "general", 0),
        ("MySQL", "mysql -e 'SHOW VARIABLES LIKE \"max_connections%\"'", "最大连接数配置", "general", 0),
        ("MySQL", "mysql -e 'SHOW PROCESSLIST'", "当前连接和查询列表", "general", 0),
        ("MySQL", "mysql -e \"SELECT COUNT(*) AS threads FROM performance_schema.threads\"", "当前线程数", "general", 0),
        ("MySQL", "mysql -e 'SHOW GLOBAL STATUS LIKE \"Uptime\"'", "MySQL 运行时长", "general", 0),
        ("MySQL", "mysql -e 'SHOW GLOBAL STATUS LIKE \"Threads_connected\"'", "当前连接数", "general", 0),
        ("MySQL", "mysql -e 'SHOW GLOBAL STATUS LIKE \"Threads_running\"'", "活跃线程数", "general", 0),
        ("MySQL", "mysql -e 'SHOW GLOBAL STATUS LIKE \"Queries\"'", "累计查询数", "general", 0),
        ("MySQL", "mysql -e 'SHOW GLOBAL STATUS LIKE \"Slow_queries\"'", "慢查询数量", "general", 0),
        ("MySQL", "mysql -e 'SHOW GLOBAL STATUS LIKE \"Bytes_received\"'", "累计接收流量", "general", 0),
        ("MySQL", "mysql -e 'SHOW GLOBAL STATUS LIKE \"Bytes_sent\"'", "累计发送流量", "general", 0),
        ("MySQL", "mysql -e \"SELECT table_schema, ROUND(SUM(data_length+index_length)/1024/1024,1) AS MB FROM information_schema.tables GROUP BY table_schema ORDER BY MB DESC LIMIT 10\"", "各库占用空间", "storage", 0),
        ("MySQL", "mysql -e \"SELECT table_schema, table_name, ROUND((data_length+index_length)/1024/1024,1) AS MB FROM information_schema.tables WHERE table_schema NOT IN ('mysql','sys','information_schema','performance_schema') ORDER BY MB DESC LIMIT 20\"", "大表排行 TOP20", "storage", 0),
        ("MySQL", "mysql -e 'SHOW VARIABLES LIKE \"datadir\"'", "数据文件存放目录", "storage", 0),
        ("MySQL", "mysql -e 'SHOW VARIABLES LIKE \"innodb_buffer_pool_size\"'", "InnoDB 缓冲池大小", "performance", 0),
        ("MySQL", "mysql -e \"SELECT * FROM sys.memory_by_host_by_current_bytes WHERE host != 'background'\"", "按主机查看内存使用", "performance", 0),
        ("MySQL", "mysql -e 'SHOW SLAVE STATUS\\G'", "主从复制状态", "ha", 0),
        ("MySQL", "mysql -e \"SHOW VARIABLES LIKE 'log_bin'\"", "binlog 是否开启", "ha", 0),
        ("MySQL", "mysql -e 'SHOW BINARY LOGS'", "binlog 文件列表", "ha", 0),
        ("MySQL", "mysql -e \"SELECT user, host, plugin FROM mysql.user\"", "用户认证方式列表", "security", 0),
        ("MySQL", "mysql -e 'SHOW VARIABLES LIKE \"innodb_flush_log_at_trx_commit\"'", "事务提交刷盘策略", "general", 0),
        // ==================== PostgreSQL ====================
        ("PostgreSQL", "psql --version", "PostgreSQL 版本", "version", 0),
        ("PostgreSQL", "psql -c 'SELECT version()'", "PostgreSQL 详细版本", "version", 0),
        ("PostgreSQL", "psql -c 'SELECT pid, usename, application_name, client_addr, state, now()-xact_start AS xact_age FROM pg_stat_activity WHERE state != $$idle$$'", "当前活跃查询", "general", 0),
        ("PostgreSQL", "psql -c 'SELECT count(*) AS total_connections FROM pg_stat_activity'", "当前连接总数", "general", 0),
        ("PostgreSQL", "psql -c \"SELECT state, count(*) FROM pg_stat_activity GROUP BY state\"", "连接状态分布", "general", 0),
        ("PostgreSQL", "psql -c \"SELECT pg_postmaster_start_time()\"", "PostgreSQL 启动时间", "general", 0),
        ("PostgreSQL", "psql -c \"SELECT datname, numbackends, xact_commit, xact_rollback, blks_read, blks_hit, tup_returned, tup_fetched, tup_inserted, tup_updated, tup_deleted FROM pg_stat_database WHERE datname NOT IN ('template0','template1')\"", "各库统计信息", "general", 0),
        ("PostgreSQL", "psql -c \"SELECT relname AS table_name, n_live_tup AS row_count FROM pg_stat_user_tables ORDER BY n_live_tup DESC LIMIT 20\"", "用户表行数 TOP20", "storage", 0),
        ("PostgreSQL", "psql -c \"SELECT datname, pg_size_pretty(pg_database_size(datname)) FROM pg_database ORDER BY pg_database_size(datname) DESC\"", "各库占用空间", "storage", 0),
        ("PostgreSQL", "psql -c \"SELECT relname, pg_size_pretty(pg_total_relation_size(relid)) FROM pg_stat_user_tables ORDER BY pg_total_relation_size(relid) DESC LIMIT 20\"", "大表占用空间 TOP20", "storage", 0),
        ("PostgreSQL", "psql -c 'SHOW data_directory'", "数据文件存放目录", "storage", 0),
        ("PostgreSQL", "psql -c \"SELECT name, setting, unit FROM pg_settings WHERE name IN ('shared_buffers','effective_cache_size','work_mem','maintenance_work_mem','wal_buffers')\"", "内存相关配置", "performance", 0),
        ("PostgreSQL", "psql -c 'SELECT * FROM pg_stat_replication'", "流复制状态", "ha", 0),
        ("PostgreSQL", "psql -c \"SELECT slot_name, slot_type, active, pg_wal_lsn_diff(pg_current_wal_lsn(), restart_lsn) AS lag_bytes FROM pg_replication_slots\"", "复制槽状态", "ha", 0),
        ("PostgreSQL", "psql -c 'SELECT * FROM pg_stat_bgwriter'", "后台写入统计", "general", 0),
        ("PostgreSQL", "psql -c \"SELECT indexrelname, idx_scan, idx_tup_read, idx_tup_fetch FROM pg_stat_user_indexes ORDER BY idx_scan DESC LIMIT 20\"", "索引使用 TOP20", "general", 0),
        ("PostgreSQL", "psql -c \"SELECT schemaname, relname, last_vacuum, last_autovacuum, n_dead_tup FROM pg_stat_user_tables ORDER BY n_dead_tup DESC LIMIT 10\"", "表膨胀/VACUUM 状态", "general", 0),
        ("PostgreSQL", "psql -c \"SELECT usename, passwd IS NOT NULL AS has_password FROM pg_shadow\"", "用户密码状态", "security", 0),
        // ==================== Oracle ====================
        ("Oracle", "sqlplus -v", "Oracle 客户端版本", "version", 0),
        ("Oracle", "echo \"SELECT BANNER FROM V\\$VERSION WHERE ROWNUM=1;\" | sqlplus -S / as sysdba", "Oracle 数据库版本", "version", 0),
        ("Oracle", "echo \"SELECT INSTANCE_NAME, HOST_NAME, VERSION, STATUS, STARTUP_TIME FROM V\\$INSTANCE;\" | sqlplus -S / as sysdba", "实例基本信息", "general", 0),
        ("Oracle", "echo \"SELECT NAME, VALUE FROM V\\$PARAMETER WHERE NAME IN ('processes','sessions','open_cursors');\" | sqlplus -S / as sysdba", "连接/进程上限", "general", 0),
        ("Oracle", "echo \"SELECT COUNT(*) FROM V\\$SESSION WHERE TYPE!='BACKGROUND';\" | sqlplus -S / as sysdba", "当前会话数", "general", 0),
        ("Oracle", "echo \"SELECT STATUS, COUNT(*) FROM V\\$SESSION GROUP BY STATUS;\" | sqlplus -S / as sysdba", "会话状态分布", "general", 0),
        ("Oracle", "echo \"SELECT NAME, ROUND(BYTES/1024/1024,1) AS MB FROM V\\$SGASTAT WHERE POOL='shared pool' AND NAME IN ('free memory','library cache','row cache') ORDER BY NAME;\" | sqlplus -S / as sysdba", "共享池使用情况", "performance", 0),
        ("Oracle", "echo \"SELECT COMPONENT, ROUND(CURRENT_SIZE/1024/1024,1) AS MB FROM V\\$SGA_DYNAMIC_COMPONENTS WHERE CURRENT_SIZE>0 ORDER BY CURRENT_SIZE DESC;\" | sqlplus -S / as sysdba", "SGA 各组件大小", "performance", 0),
        ("Oracle", "echo \"SELECT ROUND(SUM(BYTES)/1024/1024/1024,1) AS GB FROM V\\$SGAINFO WHERE NAME IN ('Fixed SGA Size','Buffer Cache Size','Shared Pool Size','Large Pool Size','Java Pool Size');\" | sqlplus -S / as sysdba", "SGA 总览", "performance", 0),
        ("Oracle", "echo \"SELECT NAME, ROUND(BYTES/1024/1024,1) AS MB FROM V\\$PGASTAT WHERE NAME IN ('total PGA allocated','total PGA inuse');\" | sqlplus -S / as sysdba", "PGA 使用情况", "performance", 0),
        ("Oracle", "echo \"SELECT TABLESPACE_NAME, ROUND(SUM(BYTES)/1024/1024/1024,1) AS TOTAL_GB, ROUND(SUM(BYTES)/1024/1024/1024-NVL(SUM(FREE_BYTES),0)/1024/1024/1024,1) AS USED_GB FROM (SELECT TABLESPACE_NAME, BYTES, 0 AS FREE_BYTES FROM DBA_DATA_FILES UNION ALL SELECT TABLESPACE_NAME, 0 AS BYTES, BYTES AS FREE_BYTES FROM DBA_FREE_SPACE) GROUP BY TABLESPACE_NAME ORDER BY TABLESPACE_NAME;\" | sqlplus -S / as sysdba", "表空间使用率", "storage", 0),
        ("Oracle", "echo \"SELECT OWNER, SEGMENT_NAME, SEGMENT_TYPE, ROUND(BYTES/1024/1024,1) AS MB FROM DBA_SEGMENTS WHERE OWNER NOT IN ('SYS','SYSTEM','SYSAUX') ORDER BY BYTES DESC FETCH FIRST 20 ROWS ONLY;\" | sqlplus -S / as sysdba", "大段 TOP20", "storage", 0),
        ("Oracle", "echo \"SELECT NAME, ROUND(TOTAL_MB,1) AS TOTAL_MB, ROUND(FREE_MB,1) AS FREE_MB FROM V\\$ASM_DISKGROUP;\" | sqlplus -S / as sysdba", "ASM 磁盘组使用情况", "storage", 0),
        ("Oracle", "echo \"SELECT TABLESPACE_NAME, STATUS FROM DBA_TABLESPACES;\" | sqlplus -S / as sysdba", "表空间状态", "storage", 0),
        ("Oracle", "echo \"SELECT NAME, SEQUENCE#, APPLIED FROM V\\$ARCHIVED_LOG WHERE DEST_ID=1 ORDER BY SEQUENCE# DESC FETCH FIRST 5 ROWS ONLY;\" | sqlplus -S / as sysdba", "归档日志最近 5 条", "general", 0),
        ("Oracle", "echo \"SELECT DEST_NAME, STATUS, TYPE FROM V\\$ARCHIVE_DEST WHERE STATUS!='INACTIVE';\" | sqlplus -S / as sysdba", "归档目标状态", "general", 0),
        ("Oracle", "echo \"SELECT DATABASE_ROLE, SWITCHOVER_STATUS FROM V\\$DATABASE;\" | sqlplus -S / as sysdba", "Data Guard 角色", "ha", 0),
        ("Oracle", "echo \"SELECT USERNAME, ACCOUNT_STATUS FROM DBA_USERS ORDER BY USERNAME;\" | sqlplus -S / as sysdba", "用户和状态", "security", 0),
        ("Oracle", "echo \"SELECT NAME, DETECTED_USAGES, FIRST_USAGE_DATE, LAST_USAGE_DATE FROM DBA_FEATURE_USAGE_STATISTICS WHERE DETECTED_USAGES > 0 ORDER BY LAST_USAGE_DATE DESC FETCH FIRST 10 ROWS ONLY;\" | sqlplus -S / as sysdba", "功能使用统计 TOP10", "general", 0),
        ("Oracle", "echo \"SELECT INSTANCE_NAME, HOST_NAME, CPU_COUNT, ROUND(TOTAL_MEM_MB/1024,1) AS MEM_GB FROM V\\$INSTANCE CROSS JOIN (SELECT SUM(BYTES)/1024/1024 AS TOTAL_MEM_MB FROM V\\$SGAINFO) CROSS JOIN (SELECT VALUE AS CPU_COUNT FROM V\\$PARAMETER WHERE NAME='cpu_count');\" | sqlplus -S / as sysdba", "宿主机 CPU/内存（Oracle 视角）", "system", 0),
        // ==================== SQL Server ====================
        ("SQL Server", "sqlcmd -Q 'SELECT @@VERSION' -W 2>/dev/null", "SQL Server 版本", "version", 0),
        ("SQL Server", "sqlcmd -Q \"SELECT name, value FROM sys.configurations WHERE name IN ('max server memory (MB)','min server memory (MB)','max degree of parallelism')\" -W", "关键配置参数", "performance", 0),
        ("SQL Server", "sqlcmd -Q \"SELECT DB_NAME(database_id) AS DB, COUNT(*) AS connections FROM sys.dm_exec_sessions GROUP BY database_id\" -W", "各库连接数", "general", 0),
        ("SQL Server", "sqlcmd -Q \"SELECT TOP 10 DB_NAME(database_id) AS DB, ROUND(SUM(size)*8/1024.0,1) AS MB FROM sys.master_files GROUP BY database_id ORDER BY MB DESC\" -W", "各库占用空间 TOP10", "storage", 0),
        ("SQL Server", "sqlcmd -Q \"SELECT database_id, DB_NAME(database_id), recovery_model_desc FROM sys.databases\" -W", "数据库恢复模式", "ha", 0),
        ("SQL Server", "sqlcmd -Q \"SELECT name, is_disabled FROM sys.sql_logins\" -W", "登录账户状态", "security", 0),
        // ==================== 达梦 (DM8) ====================
        ("达梦", "disql -v 2>/dev/null || /opt/dmdbms/bin/disql -v", "达梦版本", "version", 0),
        ("达梦", "echo \"SELECT * FROM V\\$VERSION;\" | disql SYSDBA/SYSDBA 2>/dev/null", "达梦详细版本", "version", 0),
        ("达梦", "echo \"SELECT NAME, TYPE, VALUE FROM V\\$PARAMETER WHERE NAME IN ('MAX_SESSIONS','BUFFER','MAX_BUFFER','WORKER_THREADS');\" | disql SYSDBA/SYSDBA 2>/dev/null", "关键配置参数", "general", 0),
        ("达梦", "echo \"SELECT COUNT(*) AS SESSIONS FROM V\\$SESSIONS WHERE STATE!='IDLE';\" | disql SYSDBA/SYSDBA 2>/dev/null", "当前活跃会话", "general", 0),
        ("达梦", "echo \"SELECT NAME, ROUND(TOTAL_SIZE*PAGE/1024/1024,1) AS MB FROM V\\$TABLESPACE;\" | disql SYSDBA/SYSDBA 2>/dev/null", "表空间使用情况", "storage", 0),
        ("达梦", "echo \"SELECT NAME, ROUND(TOTAL_SIZE*PAGE/1024/1024,1) AS MB, ROUND(FREE_SIZE*PAGE/1024/1024,1) AS FREE_MB FROM V\\$TABLESPACE;\" | disql SYSDBA/SYSDBA 2>/dev/null", "表空间空闲情况", "storage", 0),
        ("达梦", "echo \"SELECT T.NAME, ROUND(SUM(A.TOTAL_SIZE)*PAGE/1024/1024,1) AS MB FROM V\\$TABLESPACE T, V\\$DATAFILE A WHERE T.ID=A.GROUP_ID GROUP BY T.NAME ORDER BY MB DESC;\" | disql SYSDBA/SYSDBA 2>/dev/null", "数据文件统计", "storage", 0),
        ("达梦", "echo \"SELECT USERNAME, ACCOUNT_STATUS FROM DBA_USERS;\" | disql SYSDBA/SYSDBA 2>/dev/null", "用户状态", "security", 0),
        ("达梦", "echo \"SELECT * FROM V\\$RLOG;\" | disql SYSDBA/SYSDBA 2>/dev/null", "归档日志状态", "general", 0),
    ];

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let inserted = {
        // INSERT ... ON CONFLICT DO UPDATE：新命令插入，已存在命令仅修正 needs_root
        // （不覆盖 description/category，尊重用户可能的修改），保证全新安装与升级用户一致。
        let mut stmt = tx
            .prepare(
                "INSERT INTO command_pool (vendor, command, description, category, needs_root) \
                 SELECT ?1, ?2, ?3, ?4, ?5 \
                 WHERE NOT EXISTS (SELECT 1 FROM deleted_seed_commands WHERE vendor = ?1 AND command = ?2) \
                 ON CONFLICT(vendor, command) DO UPDATE SET needs_root = excluded.needs_root")
            .map_err(|e| e.to_string())?;
        let mut inserted = 0usize;
        for (vendor, command, description, category, needs_root) in &commands {
            let rows = stmt
                .execute(params![vendor, command, description, category, needs_root])
                .map_err(|e| format!("插入种子命令失败 ({}): {}", command, e))?;
            inserted += rows;
        }
        inserted
    };
    tx.commit().map_err(|e| e.to_string())?;
    Ok(inserted)
}
