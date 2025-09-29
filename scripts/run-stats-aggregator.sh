#!/bin/bash

# 启动 stats-aggregator 服务的脚本
# 用于测试和开发

echo "启动统计聚合服务..."

# 设置环境变量（如果未设置）
export DATABASE_URL=${DATABASE_URL:-"postgres://postgres:123456@127.0.0.1:5432/chatroom"}
export RUST_LOG=${RUST_LOG:-"info"}

echo "数据库连接: $DATABASE_URL"
echo "日志级别: $RUST_LOG"

# 切换到项目根目录
cd "$(dirname "$0")"

# 启动服务
cargo run --bin stats-aggregator