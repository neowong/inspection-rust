export const VENDORS = ["H3C", "华为", "思科", "锐捷", "飞塔", "Linux", "MySQL", "PostgreSQL", "Oracle", "SQL Server", "达梦", "Redis", "MongoDB", "其它"] as const;

export const DB_VENDORS = ["MySQL", "PostgreSQL", "Oracle", "SQL Server", "达梦", "Redis", "MongoDB"] as const;
export const OS_VENDORS = ["Linux"] as const;

export const CATEGORIES = [
  "version", "clock", "performance", "hardware", "storage", "env",
  "interface", "log", "protocol", "vpn", "ha", "security", "wireless", "general",
  "system", "disk", "network", "service", "process", "schedule",
] as const;
