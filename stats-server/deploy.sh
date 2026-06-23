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

# 1. 创建远程目录
echo "1. 创建远程目录..."
ssh $REMOTE_HOST "mkdir -p $REMOTE_DIR/data"

# 2. 复制文件
echo "2. 复制文件..."
scp -r server.js package.json Dockerfile docker-compose.yml public/ $REMOTE_HOST:$REMOTE_DIR/

# 3. 在远程服务器上构建和启动
echo "3. 构建并启动容器..."
ssh $REMOTE_HOST "cd $REMOTE_DIR && docker-compose up -d --build"

# 4. 配置 nginx 反向代理（如果需要）
echo "4. 配置 nginx..."
ssh $REMOTE_HOST "
cat > /etc/nginx/sites-available/stats << 'EOF'
server {
    listen 80;
    server_name $DOMAIN;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }
}
EOF

ln -sf /etc/nginx/sites-available/stats /etc/nginx/sites-enabled/
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
