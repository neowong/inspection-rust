const express = require('express');
const sqlite3 = require('sqlite3').verbose();
const jwt = require('jsonwebtoken');
const bcrypt = require('bcrypt');
const cors = require('cors');
const path = require('path');

const app = express();
const PORT = process.env.PORT || 3000;
const BASE_PATH = process.env.BASE_PATH || '';
const JWT_SECRET = process.env.JWT_SECRET || 'ai-inspection-stats-secret-key-change-me';

// 中间件
app.use(cors());
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
      ip TEXT,
      created_at DATETIME DEFAULT CURRENT_TIMESTAMP
    )
  `);

  db.run(`CREATE INDEX IF NOT EXISTS idx_feedbacks_created ON feedbacks(created_at)`);

  // 创建默认管理员账户（root / ai-inspection）
  const defaultPassword = process.env.ADMIN_PASSWORD || 'ai-inspection';
  bcrypt.hash(defaultPassword, 10, (err, hash) => {
    if (err) {
      console.error('密码哈希失败:', err);
      return;
    }
    db.run(
      `INSERT OR IGNORE INTO users (username, password_hash) VALUES (?, ?)`,
      ['root', hash],
      (err) => {
        if (err) {
          console.error('创建默认用户失败:', err);
        } else {
          console.log('默认管理员账户已创建: root / ' + defaultPassword);
        }
      }
    );
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

// 统计上报接口（客户端调用）
app.post(`${BASE_PATH}/api/track`, (req, res) => {
  const { device_id, version, os, timestamp } = req.body;
  const ip = req.headers['x-forwarded-for'] || req.connection.remoteAddress;

  if (!device_id || !version || !os || !timestamp) {
    return res.status(400).json({ error: '参数不完整' });
  }

  db.run(
    `INSERT INTO track_records (device_id, version, os, ip, timestamp) VALUES (?, ?, ?, ?, ?)`,
    [device_id, version, os, ip, timestamp],
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
  const limit = parseInt(req.query.limit) || 50;
  db.all(
    `SELECT device_id, version, os, ip, timestamp
     FROM track_records
     ORDER BY timestamp DESC
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

// 提交反馈（无需认证）
app.post(`${BASE_PATH}/api/feedback`, (req, res) => {
  const { device_id, feedback_type, title, content, contact, version } = req.body;
  const ip = req.headers['x-forwarded-for'] || req.connection.remoteAddress;

  if (!feedback_type || !title || !content) {
    return res.status(400).json({ error: '反馈类型、标题和内容不能为空' });
  }

  db.run(
    `INSERT INTO feedbacks (device_id, feedback_type, title, content, contact, version, ip) VALUES (?, ?, ?, ?, ?, ?, ?)`,
    [device_id, feedback_type, title, content, contact || null, version, ip],
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
  const limit = parseInt(req.query.limit) || 100;
  db.all(
    `SELECT id, device_id, feedback_type, title, content, contact, version, ip, created_at
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

// 验证令牌
app.get(`${BASE_PATH}/api/verify`, authenticateToken, (req, res) => {
  res.json({ valid: true, username: req.user.username });
});

// 启动服务器
app.listen(PORT, () => {
  console.log(`统计服务器运行在 http://localhost:${PORT}`);
  console.log(`Dashboard: http://localhost:${PORT}`);
  console.log(`默认账户: root / ai-inspection`);
});
