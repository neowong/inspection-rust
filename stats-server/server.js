const express = require('express');
const sqlite3 = require('sqlite3').verbose();
const jwt = require('jsonwebtoken');
const bcrypt = require('bcrypt');
const cors = require('cors');

// 速率限制（简易实现，避免引入额外依赖）
const rateLimitMap = new Map();
function rateLimit(key, maxRequests, windowMs) {
  return (req, res, next) => {
    const ip = req.socket.remoteAddress || 'unknown';
    const fullKey = `${key}:${ip}`;
    const now = Date.now();
    const entry = rateLimitMap.get(fullKey);
    if (!entry || now > entry.resetTime) {
      rateLimitMap.set(fullKey, { count: 1, resetTime: now + windowMs });
      return next();
    }
    entry.count++;
    if (entry.count > maxRequests) {
      return res.status(429).json({ error: '请求过于频繁，请稍后再试' });
    }
    next();
  };
}

const app = express();
const PORT = process.env.PORT || 3000;
const BASE_PATH = process.env.BASE_PATH || '';

// 强制要求 JWT_SECRET 环境变量
const JWT_SECRET = process.env.JWT_SECRET;
if (!JWT_SECRET || JWT_SECRET.length < 16) {
  console.error('错误: 请设置 JWT_SECRET 环境变量（至少 16 字符）');
  console.error('例如: JWT_SECRET=your-strong-secret docker compose up -d');
  process.exit(1);
}

// 中间件
app.use(cors({ origin: true, credentials: true }));
app.use(express.json());
app.use(BASE_PATH, express.static('public'));

// 数据库初始化
const db = new sqlite3.Database('./data/stats.db', (err) => {
  if (err) {
    console.error('数据库连接失败:', err);
    process.exit(1);
  }
  console.log('数据库连接成功');
});

// 创建表
db.serialize(() => {
  // 用户表
  db.run(`
    CREATE TABLE IF NOT EXISTS users (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      username TEXT UNIQUE NOT NULL,
      password_hash TEXT NOT NULL,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
  `);

  // 统计记录表
  db.run(`
    CREATE TABLE IF NOT EXISTS track_records (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      device_id TEXT NOT NULL,
      version TEXT NOT NULL,
      os TEXT NOT NULL,
      ip TEXT,
      timestamp DATETIME NOT NULL,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
  `);

  // 创建索引
  db.run(`CREATE INDEX IF NOT EXISTS idx_track_device_id ON track_records(device_id)`);
  db.run(`CREATE INDEX IF NOT EXISTS idx_track_timestamp ON track_records(timestamp)`);
  db.run(`CREATE INDEX IF NOT EXISTS idx_track_version ON track_records(version)`);

  // 反馈表
  db.run(`
    CREATE TABLE IF NOT EXISTS feedbacks (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      device_id TEXT,
      feedback_type TEXT NOT NULL,
      title TEXT NOT NULL,
      content TEXT NOT NULL,
      contact TEXT,
      version TEXT,
      os TEXT,
      os_version TEXT,
      ip TEXT,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
  `);

  db.run(`CREATE INDEX IF NOT EXISTS idx_feedbacks_created ON feedbacks(created_at)`);
  // 兼容旧表：添加 os/os_version 列
  db.run(`ALTER TABLE feedbacks ADD COLUMN os TEXT`, () => {});
  db.run(`ALTER TABLE feedbacks ADD COLUMN os_version TEXT`, () => {});

  // 创建管理员账户（必须通过环境变量设置密码）
  const adminPassword = process.env.ADMIN_PASSWORD;
  if (!adminPassword || adminPassword.length < 6) {
    console.error('错误: 请设置 ADMIN_PASSWORD 环境变量（至少 6 字符）');
    console.error('例如: ADMIN_PASSWORD=your-password docker compose up -d');
    process.exit(1);
  }
  // 先检查是否已有 root 用户，避免每次启动重算 bcrypt
  db.get(`SELECT id FROM users WHERE username = 'root'`, (err, row) => {
    if (err) { console.error('查询用户失败:', err); return; }
    if (row) return; // 已存在，跳过
    bcrypt.hash(adminPassword, 10, (err, hash) => {
      if (err) { console.error('密码哈希失败:', err); return; }
      db.run(
        `INSERT INTO users (username, password_hash) VALUES (?, ?)`,
        ['root', hash],
        (err) => {
          if (err) console.error('创建管理员失败:', err);
          else console.log('管理员账户已创建 (root)');
        }
      );
    });
  });
});

// JWT 认证中间件
function authenticateToken(req, res, next) {
  const authHeader = req.headers['authorization'];
  const token = authHeader && authHeader.split(' ')[1];

  if (!token) {
    return res.status(401).json({ error: '未授权' });
  }

  jwt.verify(token, JWT_SECRET, (err, user) => {
    if (err) {
      return res.status(403).json({ error: '令牌无效' });
    }
    req.user = user;
    next();
  });
}

// 登录接口
app.post(`${BASE_PATH}/api/login`, (req, res) => {
  const { username, password } = req.body;

  if (!username || !password) {
    return res.status(400).json({ error: '用户名和密码不能为空' });
  }

  db.get(
    `SELECT * FROM users WHERE username = ?`,
    [username],
    (err, user) => {
      if (err) {
        return res.status(500).json({ error: '数据库错误' });
      }
      if (!user) {
        return res.status(401).json({ error: '用户名或密码错误' });
      }

      bcrypt.compare(password, user.password_hash, (err, result) => {
        if (err || !result) {
          return res.status(401).json({ error: '用户名或密码错误' });
        }

        const token = jwt.sign(
          { id: user.id, username: user.username },
          JWT_SECRET,
          { expiresIn: '24h' }
        );

        res.json({ token, username: user.username });
      });
    }
  );
});

// 统计上报接口（客户端调用，限流 + 输入校验）
app.post(`${BASE_PATH}/api/track`, rateLimit('track', 60, 60000), (req, res) => {
  const { device_id, version, os, timestamp } = req.body;

  if (!device_id || !version || !os || !timestamp) {
    return res.status(400).json({ error: '参数不完整' });
  }
  // 输入长度限制
  if (String(device_id).length > 128 || String(version).length > 32 ||
      String(os).length > 32 || String(timestamp).length > 64) {
    return res.status(400).json({ error: '参数过长' });
  }

  // 提取客户端真实 IP（nginx 反向代理后）
  const ip = (req.headers['x-forwarded-for'] || '').split(',')[0].trim()
    || req.headers['x-real-ip']
    || req.socket.remoteAddress
    || '';

  db.run(
    `INSERT INTO track_records (device_id, version, os, ip, timestamp) VALUES (?, ?, ?, ?, ?)`,
    [String(device_id).slice(0, 128), String(version).slice(0, 32), String(os).slice(0, 32),
     String(ip).slice(0, 45), String(timestamp).slice(0, 64)],
    (err) => {
      if (err) {
        console.error('记录统计失败:', err);
        return res.status(500).json({ error: '记录失败' });
      }
      res.json({ success: true });
    }
  );
});

// Dashboard API（需要认证）

// 总览统计
app.get(`${BASE_PATH}/api/stats/overview`, authenticateToken, (req, res) => {
  const queries = {
    totalUsers: `SELECT COUNT(DISTINCT device_id) as count FROM track_records`,
    todayUsers: `SELECT COUNT(DISTINCT device_id) as count FROM track_records WHERE DATE(timestamp) = DATE('now')`,
    weekUsers: `SELECT COUNT(DISTINCT device_id) as count FROM track_records WHERE timestamp >= datetime('now', '-7 days')`,
    monthUsers: `SELECT COUNT(DISTINCT device_id) as count FROM track_records WHERE timestamp >= datetime('now', '-30 days')`,
    totalRecords: `SELECT COUNT(*) as count FROM track_records`,
  };

  const results = {};
  let completed = 0;
  const total = Object.keys(queries).length;

  Object.entries(queries).forEach(([key, sql]) => {
    db.get(sql, (err, row) => {
      results[key] = err ? 0 : (row?.count || 0);
      completed++;
      if (completed === total) {
        res.json(results);
      }
    });
  });
});

// 版本分布
app.get(`${BASE_PATH}/api/stats/versions`, authenticateToken, (req, res) => {
  db.all(
    `SELECT version, COUNT(DISTINCT device_id) as users
     FROM track_records
     GROUP BY version
     ORDER BY users DESC
     LIMIT 10`,
    (err, rows) => {
      if (err) {
        return res.status(500).json({ error: '查询失败' });
      }
      res.json(rows || []);
    }
  );
});

// 操作系统分布
app.get(`${BASE_PATH}/api/stats/os`, authenticateToken, (req, res) => {
  db.all(
    `SELECT os, COUNT(DISTINCT device_id) as users
     FROM track_records
     GROUP BY os
     ORDER BY users DESC`,
    (err, rows) => {
      if (err) {
        return res.status(500).json({ error: '查询失败' });
      }
      res.json(rows || []);
    }
  );
});

// 每日活跃用户趋势（最近30天）
app.get(`${BASE_PATH}/api/stats/daily`, authenticateToken, (req, res) => {
  db.all(
    `SELECT DATE(timestamp) as date, COUNT(DISTINCT device_id) as users
     FROM track_records
     WHERE timestamp >= datetime('now', '-30 days')
     GROUP BY DATE(timestamp)
     ORDER BY date`,
    (err, rows) => {
      if (err) {
        return res.status(500).json({ error: '查询失败' });
      }
      res.json(rows || []);
    }
  );
});

// 最近记录
app.get(`${BASE_PATH}/api/stats/recent`, authenticateToken, (req, res) => {
  const limit = Math.min(parseInt(req.query.limit) || 50, 500);
  db.all(
    `SELECT device_id, version, os, ip, timestamp
     FROM track_records
     ORDER BY timestamp DESC
     LIMIT ?`,
    [limit],
    (err, rows) => {
      if (err) { return res.status(500).json({ error: '查询失败' }); }
      res.json(rows || []);
    }
  );
});

// 提交反馈（无需认证，限流 + 输入校验）
app.post(`${BASE_PATH}/api/feedback`, rateLimit('feedback', 10, 60000), (req, res) => {
  const { device_id, feedback_type, title, content, contact, version, os, os_version } = req.body;

  if (!feedback_type || !title || !content) {
    return res.status(400).json({ error: '反馈类型、标题和内容不能为空' });
  }
  // 输入长度限制
  const validTypes = ['bug', 'feature', 'other'];
  if (!validTypes.includes(feedback_type)) {
    return res.status(400).json({ error: '无效的反馈类型' });
  }
  if (String(title).length > 200 || String(content).length > 5000 ||
      (contact && String(contact).length > 200)) {
    return res.status(400).json({ error: '输入内容过长' });
  }

  db.run(
    `INSERT INTO feedbacks (device_id, feedback_type, title, content, contact, version, os, os_version) VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
    [String(device_id || '').slice(0, 128), feedback_type,
     String(title).slice(0, 200), String(content).slice(0, 5000),
     contact ? String(contact).slice(0, 200) : null, String(version || '').slice(0, 32),
     String(os || '').slice(0, 32), String(os_version || '').slice(0, 64)],
    (err) => {
      if (err) {
        console.error('记录反馈失败:', err);
        return res.status(500).json({ error: '记录失败' });
      }
      res.json({ success: true });
    }
  );
});

// 获取反馈列表（需认证）
app.get(`${BASE_PATH}/api/feedbacks`, authenticateToken, (req, res) => {
  const limit = Math.min(parseInt(req.query.limit) || 100, 500);
  db.all(
    `SELECT id, device_id, feedback_type, title, content, contact, version, os, os_version, created_at
     FROM feedbacks
     ORDER BY created_at DESC
     LIMIT ?`,
    [limit],
    (err, rows) => {
      if (err) {
        return res.status(500).json({ error: '查询失败' });
      }
      res.json(rows || []);
    }
  );
});

// 健康检查（无需认证，供 docker healthcheck 用）
app.get(`${BASE_PATH}/api/health`, (req, res) => {
  res.json({ status: 'ok' });
});

// 验证令牌
app.get(`${BASE_PATH}/api/verify`, authenticateToken, (req, res) => {
  res.json({ valid: true, username: req.user.username });
});

// 优雅关闭
process.on('SIGTERM', () => { db.close(); process.exit(0); });
process.on('SIGINT', () => { db.close(); process.exit(0); });

// 启动服务器
app.listen(PORT, () => {
  console.log(`统计服务器运行在 http://localhost:${PORT}`);
  console.log(`Dashboard: http://localhost:${PORT}${BASE_PATH}/`);
  console.log(`管理员账户: root (密码见 ADMIN_PASSWORD 环境变量)`);
});
