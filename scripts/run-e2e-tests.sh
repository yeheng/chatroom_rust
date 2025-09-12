#!/bin/bash

# 端到端测试和性能测试运行脚本
# 
# 这个脚本运行所有的端到端测试和性能测试，生成详细的测试报告

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查Docker是否运行（测试容器需要）
check_docker() {
    log_info "检查Docker是否运行..."
    if ! docker info > /dev/null 2>&1; then
        log_error "Docker未运行或无法访问，请启动Docker"
        exit 1
    fi
    log_success "Docker检查通过"
}

# 清理函数
cleanup() {
    log_info "清理测试环境..."
    # 停止可能正在运行的测试容器
    docker stop $(docker ps -aq --filter "ancestor=postgres" --filter "ancestor=redis") 2>/dev/null || true
    docker rm $(docker ps -aq --filter "ancestor=postgres" --filter "ancestor=redis") 2>/dev/null || true
    log_info "清理完成"
}

# 信号处理，确保退出时清理
trap cleanup EXIT

# 运行单元测试
run_unit_tests() {
    log_info "运行单元测试..."
    
    echo "==================== 单元测试 ===================="
    if cargo test --workspace --lib --bins; then
        log_success "单元测试通过"
    else
        log_error "单元测试失败"
        return 1
    fi
}

# 运行集成测试
run_integration_tests() {
    log_info "运行集成测试..."
    
    echo "==================== 集成测试 ===================="
    if cargo test -p tests integration_test; then
        log_success "集成测试通过"
    else
        log_error "集成测试失败"
        return 1
    fi
    
    if cargo test -p tests jwt_integration; then
        log_success "JWT集成测试通过"
    else
        log_error "JWT集成测试失败"
        return 1
    fi
}

# 运行端到端测试
run_e2e_tests() {
    log_info "运行端到端测试..."
    
    echo "==================== 端到端测试 ===================="
    
    # 设置测试环境变量
    export RUST_LOG=info
    export TEST_TIMEOUT=300  # 5分钟超时
    
    # 运行所有端到端测试
    if cargo test -p tests e2e_tests -- --test-threads=1 --nocapture; then
        log_success "端到端测试通过"
    else
        log_error "端到端测试失败"
        return 1
    fi
}

# 运行性能测试
run_performance_tests() {
    log_info "运行性能测试..."
    
    echo "==================== 性能测试 ===================="
    
    # 设置性能测试环境变量
    export RUST_LOG=info
    export TEST_TIMEOUT=600  # 10分钟超时
    export PERF_TEST_MODE=1
    
    # 运行性能测试（单线程执行以获得准确的性能指标）
    if cargo test -p tests performance_tests -- --test-threads=1 --nocapture; then
        log_success "性能测试通过"
    else
        log_warning "性能测试失败或未达到性能要求"
        # 性能测试失败不应该阻止构建，只是警告
    fi
}

# 生成测试报告
generate_report() {
    log_info "生成测试报告..."
    
    REPORT_DIR="test-reports"
    mkdir -p "$REPORT_DIR"
    
    # 生成覆盖率报告（如果有tarpaulin）
    if command -v cargo-tarpaulin &> /dev/null; then
        log_info "生成代码覆盖率报告..."
        cargo tarpaulin --out Html --output-dir "$REPORT_DIR" || log_warning "覆盖率报告生成失败"
    else
        log_warning "未安装cargo-tarpaulin，跳过覆盖率报告"
    fi
    
    # 生成性能基准报告
    log_info "生成性能基准报告..."
    echo "# 性能测试报告" > "$REPORT_DIR/performance-report.md"
    echo "" >> "$REPORT_DIR/performance-report.md"
    echo "测试时间: $(date)" >> "$REPORT_DIR/performance-report.md"
    echo "" >> "$REPORT_DIR/performance-report.md"
    echo "## 测试结果摘要" >> "$REPORT_DIR/performance-report.md"
    echo "" >> "$REPORT_DIR/performance-report.md"
    echo "详细的性能指标请查看测试输出日志。" >> "$REPORT_DIR/performance-report.md"
    
    log_success "测试报告生成完成，位于 $REPORT_DIR 目录"
}

# 主函数
main() {
    echo "=========================================="
    echo "    聊天室系统 - 端到端测试和性能测试    "
    echo "=========================================="
    
    # 检查先决条件
    check_docker
    
    # 记录开始时间
    START_TIME=$(date +%s)
    
    # 运行测试套件
    run_unit_tests || exit 1
    run_integration_tests || exit 1
    run_e2e_tests || exit 1
    run_performance_tests  # 不因性能测试失败而退出
    
    # 生成报告
    generate_report
    
    # 计算总耗时
    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))
    
    echo "=========================================="
    log_success "所有测试完成！总耗时: ${DURATION}秒"
    echo "=========================================="
}

# 帮助信息
show_help() {
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  --unit-only     仅运行单元测试"
    echo "  --integration   仅运行集成测试"
    echo "  --e2e-only      仅运行端到端测试"
    echo "  --perf-only     仅运行性能测试"
    echo "  --no-cleanup    测试后不清理环境"
    echo "  --help          显示此帮助信息"
    echo ""
    echo "示例:"
    echo "  $0                    # 运行所有测试"
    echo "  $0 --e2e-only         # 仅运行端到端测试"
    echo "  $0 --perf-only        # 仅运行性能测试"
}

# 解析命令行参数
case "$1" in
    --unit-only)
        check_docker
        run_unit_tests
        ;;
    --integration)
        check_docker
        run_integration_tests
        ;;
    --e2e-only)
        check_docker
        run_e2e_tests
        ;;
    --perf-only)
        check_docker
        run_performance_tests
        ;;
    --no-cleanup)
        trap - EXIT
        main
        ;;
    --help)
        show_help
        ;;
    "")
        main
        ;;
    *)
        echo "未知选项: $1"
        show_help
        exit 1
        ;;
esac