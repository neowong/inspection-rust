# AI巡检助手统计服务端

## 功能

- 接收客户端匿名统计上报
- JWT 认证保护 Dashboard
- SQLite 存储统计数据
- 可视化 Dashboard（用户数、版本分布、OS 分布、趋势图）

## 快速开始

### 本地开发

```bash
cd stats-server
npm install
npm run dev
```

访问 http://localhost:3000，默认账户: root / ai-inspection

### Docker 部署

```bash
cd stats-server
docker-compose up -d --build
```

### 生产部署

```bash
# 1. 复制到服务器
scp -r stats-server/ root@neowong.eu.org:/opt/ai-inspection-stats/

# 2. 在服务器上执行
ssh root@neowong.eu.org
cd /opt/ai-inspection-stats
docker-compose up -d --build

# 3. 配置 nginx 反向代理（见 deploy.sh）
```

## API 接口

### 统计上报（客户端调用）

```
POST /api/track
Content-Type: application/json

{
  "device_id": "匿名设备ID（SHA-256哈希）",
  "version": "3.40.18",
  "os": "windows",
  "timestamp": "2026-06-23T10:00:00Z"
}
```

### 登录认证

```
POST /api/login
Content-Type: application/json

{
  "username": "root",
  "password": "ai-inspection"
}

Response: { "token": "jwt_token", "username": "root" }
```

### Dashboard API（需要认证）

```
GET /api/stats/overview      # 总览统计
GET /api/stats/versions      # 版本分布
GET /api/stats/os            # 操作系统分布
GET /api/stats/daily         # 每日活跃用户（最近30天）
GET /api/stats/recent?limit=50  # 最近记录
```

## 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| PORT | 服务端口 | 3000 |
| JWT_SECRET | JWT 密钥 | ai-inspection-stats-secret-key-change-me |
| ADMIN_PASSWORD | 管理员密码 | ai-inspection |

## 安全说明

- Dashboard 需要 JWT 认证
- 统计上报接口无需认证（公开）
- 客户端 IP 从请求头提取（X-Forwarded-For / Remote-Addr）
- 匿名 device_id 为 SHA-256 哈希，无法反推原始信息
- HTTPS 由 nginx 反向代理处理

## 数据隐私

- 不收集：IP 地址、用户名、设备数据、巡检内容
- 收集：匿名 device_id、版本号、OS、时间戳
- 用途：统计用户规模、版本分布、平台分布
