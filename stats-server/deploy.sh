#!/bin/bash

# AI巡检助手统计服务端部署脚本
# 部署到 neowong.eu.org

set -e

echo "=========================================="
echo "  AI巡检助手统计服务端部署"
echo "=========================================="

# 配置
REMOTE_HOST="root@neowong.eu.org"
REMOTE_DIR="/opt/ai-inspection-stats"
DOMAIN="neowong.eu.org"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 1. 创建远程目录
echo "1. 创建远程目录..."
ssh $REMOTE_HOST "mkdir -p $REMOTE_DIR/data"

# 2. 复制文件
echo "2. 复制文件..."
scp -r $SCRIPT_DIR/server.js $SCRIPT_DIR/package.json $SCRIPT_DIR/Dockerfile $SCRIPT_DIR/docker-compose.yml $SCRIPT_DIR/public/ $REMOTE_HOST:$REMOTE_DIR/

# 3. 创建 .env 文件（第一次部署时生成随机密码）
echo "3. 检查环境变量..."
ssh $REMOTE_HOST "
cd $REMOTE_DIR
if [ ! -f .env ]; then
  JWT_SECRET=\$(openssl rand -hex 24)
  ADMIN_PASSWORD=\$(openssl rand -hex 12)
  cat > .env << ENVEOF
JWT_SECRET=\$JWT_SECRET
ADMIN_PASSWORD=\$ADMIN_PASSWORD
ENVEOF
  echo '=== 请保存以下凭据 ==='
  echo \"管理员: root / \$ADMIN_PASSWORD\"
  echo \"JWT密钥: \$JWT_SECRET\"
  echo '========================'
fi
"

# 4. 构建并启动容器
echo "4. 构建并启动容器..."
ssh $REMOTE_HOST "cd $REMOTE_DIR && docker compose up -d --build"

# 4. 配置 nginx 反向代理（/stats 子路径）
echo "4. 配置 nginx..."
ssh $REMOTE_HOST "
# 检查是否已有 nginx 配置
if [ -f /etc/nginx/sites-enabled/default ]; then
  # 在现有配置中添加 /stats location
  if ! grep -q 'location /stats' /etc/nginx/sites-enabled/default; then
    # 在 server 块的最后一个 } 前插入 location
    sed -i '/^    }/i\\
    location /stats {\\
        proxy_pass http://localhost:3000/stats;\\
        proxy_set_header Host \$host;\\
        proxy_set_header X-Real-IP \$remote_addr;\\
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;\\
        proxy_set_header X-Forwarded-Proto \$scheme;\\
    }' /etc/nginx/sites-enabled/default
  fi
else
  # 创建新的 nginx 配置
  cat > /etc/nginx/sites-available/stats << 'EOF'
server {
    listen 80;
    server_name $DOMAIN;

    location /stats {
        proxy_pass http://localhost:3000/stats;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }
}
EOF
  ln -sf /etc/nginx/sites-available/stats /etc/nginx/sites-enabled/
fi

nginx -t && systemctl reload nginx
"

echo "=========================================="
echo "  部署完成！"
echo "=========================================="
echo ""
echo "访问地址: http://$DOMAIN"
echo "默认账户: root"
echo "默认密码: ai-inspection"
echo ""
echo "请立即修改默认密码！"
echo "=========================================="
