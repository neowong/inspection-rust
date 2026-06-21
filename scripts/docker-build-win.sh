#!/bin/bash
# Docker/Podman 容器内执行：npm ci → 前端 build → Rust 交叉编译 → Windows exe
set -euo pipefail

cd /build

echo "=== 安装前端依赖 ==="
npm ci

echo "=== 构建前端 ==="
npm run build

echo "=== 交叉编译 Windows 二进制（x86_64-pc-windows-gnu） ==="
cd src-tauri
cargo build --target x86_64-pc-windows-gnu --release

echo "=== 完成 ==="
exe="target/x86_64-pc-windows-gnu/release/inspection-rust.exe"
if [ -f "$exe" ]; then
    size=$(du -h "$exe" | cut -f1)
    echo "输出: src-tauri/$exe ($size)"
else
    echo "错误: 未找到输出文件"
    exit 1
fi
