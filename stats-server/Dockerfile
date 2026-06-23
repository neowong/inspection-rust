FROM node:18-alpine

# 创建工作目录
WORKDIR /app

# 复制 package.json
COPY package.json .

# 安装依赖
RUN npm install --production

# 复制源代码
COPY server.js .
COPY public ./public

# 创建数据目录
RUN mkdir -p /app/data

# 暴露端口
EXPOSE 3000

# 启动命令
CMD ["node", "server.js"]
