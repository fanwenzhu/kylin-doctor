#!/usr/bin/env bash
#
# kylin-doctor 一键安装脚本
# 银河麒麟桌面系统自我诊断工具
#
# 用法:
#   curl -fsSL https://raw.githubusercontent.com/fanwenzhu/kylin-doctor/master/install.sh | sudo bash
#   或
#   chmod +x install.sh && sudo ./install.sh
#
# 选项:
#   --skip-deps       跳过依赖安装
#   --skip-rust       跳过 Rust 安装
#   --skip-ollama     跳过 Ollama 安装
#   --with-ollama     自动安装 Ollama 和推荐模型
#   --fix-deps        自动修复依赖版本冲突
#   --prefix <path>   安装目录 (默认: /usr/local)
#   --branch <name>   Git 分支 (默认: master)
#   --log <path>      日志文件路径 (默认: /var/log/kylin-doctor-install.log)
#   --help            显示帮助信息

set -euo pipefail

# ============================================================
# 颜色与符号
# ============================================================
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

OK="${GREEN}✓${NC}"
WARN="${YELLOW}⚠${NC}"
ERR="${RED}✗${NC}"
INFO="${CYAN}ℹ${NC}"

# ============================================================
# 默认配置
# ============================================================
REPO_URL="https://github.com/fanwenzhu/kylin-doctor.git"
BRANCH="master"
INSTALL_PREFIX="/usr/local"
BUILD_DIR="/tmp/kylin-doctor-build-$$"
SKIP_DEPS=false
SKIP_RUST=false
SKIP_OLLAMA=true  # 默认不安装 Ollama
WITH_OLLAMA=false
FIX_DEPS=false
LOG_FILE="/var/log/kylin-doctor-install.log"

# ============================================================
# 日志系统
# ============================================================

# 初始化日志文件
init_log() {
    local log_dir
    log_dir=$(dirname "$LOG_FILE")
    mkdir -p "$log_dir" 2>/dev/null || true

    # 如果无法写入默认位置，使用备用位置
    if ! touch "$LOG_FILE" 2>/dev/null; then
        LOG_FILE="/tmp/kylin-doctor-install-$$.log"
        mkdir -p "$(dirname "$LOG_FILE")"
    fi

    {
        echo "========================================"
        echo "kylin-doctor 安装日志"
        echo "开始时间: $(date '+%Y-%m-%d %H:%M:%S')"
        echo "系统信息: $(uname -a)"
        echo "========================================"
    } > "$LOG_FILE"
}

# 记录到日志文件（不输出到终端）
log_to_file() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*" >> "$LOG_FILE"
}

# 带日志的输出函数
log_info() {
    echo -e "  ${INFO} $*"
    log_to_file "INFO: $*"
}

log_ok() {
    echo -e "  ${OK} $*"
    log_to_file "OK: $*"
}

log_warn() {
    echo -e "  ${WARN} $*"
    log_to_file "WARN: $*"
}

log_err() {
    echo -e "  ${ERR} $*"
    log_to_file "ERROR: $*"
}

log_step() {
    echo -e "\n${BOLD}${CYAN}[$1/6]${NC} ${BOLD}$*${NC}"
    log_to_file "STEP: [$1/6] $*"
}

# 执行命令并记录输出（终端只显示简要状态，详细输出到日志）
run_cmd() {
    local desc="$1"
    shift
    log_info "$desc..."
    log_to_file "CMD: $*"

    local output
    if output=$("$@" 2>&1); then
        echo "$output" >> "$LOG_FILE"
        return 0
    else
        local rc=$?
        echo "$output" >> "$LOG_FILE"
        log_err "$desc 失败 (退出码: $rc)"
        log_to_file "FAILED with exit code $rc"
        return $rc
    fi
}

# 执行命令，失败时仅警告不退出
run_cmd_warn() {
    local desc="$1"
    shift
    log_info "$desc..."
    log_to_file "CMD: $*"

    local output
    if output=$("$@" 2>&1); then
        echo "$output" >> "$LOG_FILE"
        return 0
    else
        local rc=$?
        echo "$output" >> "$LOG_FILE"
        log_warn "$desc 失败 (退出码: $rc)，继续安装..."
        log_to_file "WARN: $desc failed with exit code $rc, continuing..."
        return 0
    fi
}

# ============================================================
# 错误处理
# ============================================================

# 安装失败时的清理和提示
cleanup_on_error() {
    local rc=$?
    echo ""
    echo -e "${RED}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${RED}${BOLD}  ✗ 安装失败 (退出码: $rc)${NC}"
    echo -e "${RED}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo "  日志文件: $LOG_FILE"
    echo ""
    echo "  查看详细错误:"
    echo "    tail -50 $LOG_FILE"
    echo ""
    echo "  常见问题解决:"
    echo "    1. 依赖版本冲突:"
    echo "       sudo ./install.sh --fix-deps"
    echo ""
    echo "    2. 网络问题 (跳过 Ollama):"
    echo "       sudo ./install.sh --skip-ollama"
    echo ""
    echo "    3. 编译失败 (跳过 Rust 安装):"
    echo "       sudo ./install.sh --skip-rust"
    echo ""
    echo "    4. 手动安装参考:"
    echo "       https://github.com/fanwenzhu/kylin-doctor/blob/master/docs/DEPLOYMENT.md"
    echo ""
    log_to_file "安装失败，退出码: $rc"
    exit $rc
}

trap cleanup_on_error ERR

# 带提示的失败退出
fail_with_hint() {
    local msg="$1"
    local hint="${2:-}"
    log_err "$msg"
    if [[ -n "$hint" ]]; then
        echo ""
        echo "  ${BOLD}解决建议:${NC} $hint"
    fi
    exit 1
}

# ============================================================
# 辅助函数
# ============================================================

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_err "此脚本需要 root 权限运行"
        echo ""
        echo "  请使用: sudo $0 $*"
        echo "  或:     curl -fsSL $REPO_URL/raw/master/install.sh | sudo bash"
        exit 1
    fi
}

detect_os() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        OS_ID="${ID:-unknown}"
        OS_VERSION="${VERSION_ID:-unknown}"
        OS_NAME="${PRETTY_NAME:-$OS_ID $OS_VERSION}"
    elif [[ -f /etc/kylin-version ]]; then
        OS_ID="kylin"
        OS_VERSION=$(cat /etc/kylin-version 2>/dev/null || echo "unknown")
        OS_NAME="Kylin $OS_VERSION"
    else
        OS_ID="unknown"
        OS_VERSION="unknown"
        OS_NAME="Unknown Linux"
    fi

    ARCH=$(uname -m)
    case "$ARCH" in
        x86_64|amd64) ARCH="x86_64" ;;
        aarch64|arm64) ARCH="aarch64" ;;
        *)
            log_err "不支持的架构: $ARCH"
            exit 1
            ;;
    esac

    log_to_file "OS: $OS_NAME ($OS_ID $OS_VERSION), ARCH: $ARCH"
}

detect_pkg_manager() {
    if command -v apt-get &>/dev/null; then
        PKG_MANAGER="apt"
        PKG_UPDATE="apt-get update -qq"
        PKG_INSTALL="apt-get install -y -qq"
    elif command -v dnf &>/dev/null; then
        PKG_MANAGER="dnf"
        PKG_UPDATE="dnf makecache -q"
        PKG_INSTALL="dnf install -y -q"
    elif command -v yum &>/dev/null; then
        PKG_MANAGER="yum"
        PKG_UPDATE="yum makecache -q"
        PKG_INSTALL="yum install -y -q"
    elif command -v pacman &>/dev/null; then
        PKG_MANAGER="pacman"
        PKG_UPDATE="pacman -Sy"
        PKG_INSTALL="pacman -S --noconfirm"
    else
        PKG_MANAGER="unknown"
        log_warn "未检测到包管理器，跳过依赖安装"
    fi

    log_to_file "包管理器: $PKG_MANAGER"
}

# ============================================================
# 依赖冲突修复
# ============================================================

# 修复 libssl-dev 版本冲突 (Kylin 常见问题)
fix_libssl_dev() {
    if [[ "$PKG_MANAGER" != "apt" ]]; then
        log_info "非 apt 系统，跳过 libssl-dev 检查"
        return 0
    fi

    log_info "检查 libssl-dev 依赖..."

    # 检查是否已正确安装
    if dpkg -l libssl-dev 2>/dev/null | grep -q "^ii"; then
        log_ok "libssl-dev 已正确安装"
        return 0
    fi

    # 检查是否有版本冲突
    local test_output
    if test_output=$(apt-get install -y --dry-run libssl-dev 2>&1); then
        log_ok "libssl-dev 依赖正常"
        return 0
    fi

    # 检测版本冲突特征
    if echo "$test_output" | grep -qE "版本不匹配|but [0-9]|is to be installed|Depends:|however"; then
        log_warn "检测到 libssl-dev 版本冲突"
        log_to_file "冲突详情: $test_output"

        echo ""
        echo "  ${YELLOW}┌─────────────────────────────────────────────────────┐${NC}"
        echo "  ${YELLOW}│  检测到 libssl-dev 依赖版本冲突 (Kylin 常见问题)   │${NC}"
        echo "  ${YELLOW}└─────────────────────────────────────────────────────┘${NC}"
        echo ""

        # 方案1: 尝试修复依赖
        log_info "尝试方案1: 修复依赖关系..."
        if apt --fix-broken install -y >> "$LOG_FILE" 2>&1; then
            # 再次检查
            if dpkg -l libssl-dev 2>/dev/null | grep -q "^ii"; then
                log_ok "依赖修复成功"
                return 0
            fi
        fi

        # 方案2: 强制安装
        log_info "尝试方案2: 强制安装 libssl-dev..."
        local tmp_dir
        tmp_dir=$(mktemp -d)
        (
            cd "$tmp_dir"
            apt-get download libssl-dev >> "$LOG_FILE" 2>&1 || true
            local deb_file
            deb_file=$(ls -t libssl-dev*.deb 2>/dev/null | head -1)
            if [[ -n "$deb_file" ]]; then
                dpkg --force-depends -i "$deb_file" >> "$LOG_FILE" 2>&1
                rm -f "$deb_file"
            fi
        )
        rm -rf "$tmp_dir"

        # 验证
        if dpkg -l libssl-dev 2>/dev/null | grep -q "^ii"; then
            log_ok "libssl-dev 已强制安装"
            echo ""
            echo "  ${YELLOW}注意: 强制安装可能导致后续 apt 操作报依赖警告${NC}"
            echo "  ${YELLOW}如需恢复: sudo dpkg --purge --force-depends libssl-dev${NC}"
            echo "  ${YELLOW}          sudo apt --fix-broken install${NC}"
            echo ""
            log_to_file "WARNING: libssl-dev force-installed with --force-depends"
            return 0
        fi

        # 方案3: 跳过
        log_warn "自动修复失败"
        echo ""
        echo "  可选操作:"
        echo "    1. 手动强制安装:"
        echo "       apt-get download libssl-dev"
        echo "       sudo dpkg --force-depends -i libssl-dev*.deb"
        echo ""
        echo "    2. 跳过编译依赖 (使用预编译版本):"
        echo "       sudo ./install.sh --skip-deps --skip-rust"
        echo ""
        echo "    3. 继续安装 (编译可能失败):"
        read -p "  是否继续安装? [y/N] " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
        return 0
    fi

    log_ok "libssl-dev 检查通过"
    return 0
}

# 检查并安装 zstd (Ollama 依赖)
check_zstd() {
    if command -v zstd &>/dev/null; then
        log_ok "zstd 已安装"
        return 0
    fi

    log_info "安装 zstd (Ollama 依赖)..."
    run_cmd "安装 zstd" $PKG_INSTALL zstd

    if ! command -v zstd &>/dev/null; then
        log_warn "zstd 安装失败，Ollama 可能无法正常工作"
        return 1
    fi
    return 0
}

# ============================================================
# 参数解析
# ============================================================

show_help() {
    cat << 'EOF'
kylin-doctor 一键安装脚本

用法:
  sudo ./install.sh [选项]

选项:
  --skip-deps       跳过系统依赖安装
  --skip-rust       跳过 Rust 工具链安装
  --skip-ollama     跳过 Ollama 安装 (默认)
  --with-ollama     自动安装 Ollama 和推荐模型
  --fix-deps        自动修复依赖版本冲突 (Kylin 推荐)
  --prefix <path>   安装目录 (默认: /usr/local)
  --branch <name>   Git 分支 (默认: master)
  --log <path>      日志文件路径 (默认: /var/log/kylin-doctor-install.log)
  --help            显示此帮助信息

示例:
  # 基础安装
  sudo ./install.sh

  # 首次安装 (推荐，自动修复依赖)
  sudo ./install.sh --fix-deps

  # 安装并配置 AI 模型
  sudo ./install.sh --with-ollama

  # 自定义安装目录
  sudo ./install.sh --prefix /opt/kylin-doctor

  # 跳过依赖 (已安装过)
  sudo ./install.sh --skip-deps

日志文件:
  安装过程会记录到 /var/log/kylin-doctor-install.log
  排查问题时请查看: tail -50 /var/log/kylin-doctor-install.log
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --skip-deps)    SKIP_DEPS=true ;;
            --skip-rust)    SKIP_RUST=true ;;
            --skip-ollama)  SKIP_OLLAMA=true ;;
            --with-ollama)  WITH_OLLAMA=true; SKIP_OLLAMA=false ;;
            --fix-deps)     FIX_DEPS=true ;;
            --prefix)
                shift
                INSTALL_PREFIX="${1:-/usr/local}"
                ;;
            --branch)
                shift
                BRANCH="${1:-master}"
                ;;
            --log)
                shift
                LOG_FILE="${1:-/var/log/kylin-doctor-install.log}"
                ;;
            --help|-h)      show_help; exit 0 ;;
            *)
                log_err "未知选项: $1"
                show_help
                exit 1
                ;;
        esac
        shift
    done
}

# ============================================================
# 安装步骤
# ============================================================

step_1_check_environment() {
    log_step 1 "检查系统环境"

    detect_os
    detect_pkg_manager

    echo "  操作系统: ${BOLD}$OS_NAME${NC}"
    echo "  系统架构: ${BOLD}$ARCH${NC}"
    echo "  包管理器: ${BOLD}$PKG_MANAGER${NC}"
    echo "  安装目录: ${BOLD}$INSTALL_PREFIX${NC}"
    echo "  Git 分支: ${BOLD}$BRANCH${NC}"
    echo "  日志文件: ${BOLD}$LOG_FILE${NC}"

    # 检查必要命令
    local missing=()
    for cmd in git curl; do
        if ! command -v "$cmd" &>/dev/null; then
            missing+=("$cmd")
        fi
    done

    if [[ ${#missing[@]} -gt 0 ]]; then
        log_warn "缺少必要命令: ${missing[*]}"
        log_info "尝试自动安装..."
        run_cmd "更新软件源" $PKG_UPDATE || true
        run_cmd "安装缺失命令" $PKG_INSTALL "${missing[@]}" || {
            fail_with_hint "安装失败，请手动安装: ${missing[*]}" \
                "sudo $PKG_INSTALL ${missing[*]}"
        }
        log_ok "已安装: ${missing[*]}"
    fi

    # 检查磁盘空间 (需要约 1GB)
    local avail_mb
    avail_mb=$(df -m /tmp 2>/dev/null | awk 'NR==2{print $4}' || echo "0")
    if [[ "$avail_mb" -lt 500 ]]; then
        log_warn "/tmp 可用空间不足 500MB (当前: ${avail_mb}MB)"
        log_info "建议清理 /tmp 后重试"
    fi

    log_ok "环境检查通过"
}

step_2_install_deps() {
    log_step 2 "安装系统依赖"

    if $SKIP_DEPS; then
        log_info "跳过依赖安装 (--skip-deps)"
        return
    fi

    if [[ "$PKG_MANAGER" == "unknown" ]]; then
        log_warn "未知包管理器，请手动安装以下依赖:"
        echo "    procps coreutils pciutils usbutils smartmontools"
        echo "    dmidecode lm-sensors iproute2 iputils-ping fontconfig"
        return
    fi

    log_info "更新软件源..."
    run_cmd "更新软件源" $PKG_UPDATE || true

    # 通用依赖列表 (每个包先检查是否存在)
    local packages=(
        procps coreutils
        pciutils usbutils smartmontools
        dmidecode lm-sensors
        iproute2 iputils-ping
        fontconfig
        build-essential pkg-config libssl-dev
    )

    # 根据包管理器调整包名
    if [[ "$PKG_MANAGER" == "pacman" ]]; then
        packages=(
            procps-ng coreutils
            pciutils usbutils smartmontools
            dmidecode lm_sensors
            iproute2 iputils
            fontconfig
            base-devel openssl
        )
    fi

    local to_install=()
    for pkg in "${packages[@]}"; do
        # 简单检查: 命令是否存在或包是否已安装
        case "$pkg" in
            build-essential|base-devel)
                if ! command -v gcc &>/dev/null; then
                    to_install+=("$pkg")
                fi
                ;;
            pkg-config)
                if ! command -v pkg-config &>/dev/null; then
                    to_install+=("$pkg")
                fi
                ;;
            libssl-dev|openssl)
                # 总是尝试安装，编译需要
                to_install+=("$pkg")
                ;;
            *)
                to_install+=("$pkg")
                ;;
        esac
    done

    if [[ ${#to_install[@]} -gt 0 ]]; then
        log_info "安装依赖包 (${#to_install[@]} 个)..."
        run_cmd_warn "安装系统依赖" $PKG_INSTALL "${to_install[@]}"
    fi

    log_ok "依赖安装完成"
}

step_3_install_rust() {
    log_step 3 "安装 Rust 工具链"

    if $SKIP_RUST; then
        log_info "跳过 Rust 安装 (--skip-rust)"
    elif command -v rustc &>/dev/null; then
        local rust_ver
        rust_ver=$(rustc --version 2>/dev/null | awk '{print $2}')
        log_ok "Rust 已安装 (版本: $rust_ver)"

        # 检查版本是否足够新 (需要 1.70+)
        local major minor
        major=$(echo "$rust_ver" | cut -d. -f1)
        minor=$(echo "$rust_ver" | cut -d. -f2)
        if [[ "$major" -lt 1 ]] || { [[ "$major" -eq 1 ]] && [[ "$minor" -lt 70 ]]; }; then
            log_warn "Rust 版本过低 ($rust_ver)，需要 1.70+，正在更新..."
            run_cmd "更新 Rust" rustup update stable || {
                fail_with_hint "Rust 更新失败" \
                    "请手动运行: rustup update stable"
            }
        fi
    else
        log_info "安装 Rust 工具链..."
        # 设置非交互模式
        export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.cargo}"
        export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"

        log_to_file "安装 Rust 到 $CARGO_HOME"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
            sh -s -- -y --default-toolchain stable >> "$LOG_FILE" 2>&1

        if [[ -f "$CARGO_HOME/env" ]]; then
            source "$CARGO_HOME/env"
        elif [[ -f "$HOME/.cargo/env" ]]; then
            source "$HOME/.cargo/env"
        fi

        if ! command -v rustc &>/dev/null; then
            fail_with_hint "Rust 安装失败" \
                "请手动安装: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        fi

        log_ok "Rust 安装完成 ($(rustc --version))"
    fi

    # 确保 cargo 在 PATH 中
    export PATH="$HOME/.cargo/bin:$PATH"
    log_to_file "PATH: $PATH"
}

step_4_build_install() {
    log_step 4 "编译并安装 kylin-doctor"

    # 清理旧的构建目录
    rm -rf "$BUILD_DIR"

    log_info "克隆仓库..."
    run_cmd "克隆 $BRANCH 分支" git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$BUILD_DIR"

    cd "$BUILD_DIR"

    log_info "编译项目 (release 模式，可能需要几分钟)..."
    echo ""

    # 编译，输出到日志
    log_to_file "开始 cargo build --release"
    if cargo build --release >> "$LOG_FILE" 2>&1; then
        log_ok "编译成功"
    else
        fail_with_hint "编译失败" \
            "查看详细错误: grep -A5 'error\\[' $LOG_FILE"
    fi
    echo ""

    # 检查编译产物
    local bin_dir="$BUILD_DIR/target/release"
    local cli_bin="$bin_dir/kylin-doctor"
    local web_bin="$bin_dir/kylin-doctor-web"

    if [[ ! -f "$cli_bin" ]]; then
        fail_with_hint "CLI 二进制编译失败" \
            "检查编译日志: tail -100 $LOG_FILE"
    fi
    log_ok "CLI 编译成功"

    if [[ ! -f "$web_bin" ]]; then
        log_warn "Web 二进制编译失败 (非致命)"
    else
        log_ok "Web 二进制编译成功"
    fi

    # 运行测试
    log_info "运行测试..."
    if cargo test --quiet >> "$LOG_FILE" 2>&1; then
        log_ok "所有测试通过"
    else
        log_warn "部分测试失败 (非致命，继续安装)"
        log_to_file "WARN: 部分测试失败"
    fi

    # 安装二进制文件
    log_info "安装到 $INSTALL_PREFIX/bin/ ..."
    mkdir -p "$INSTALL_PREFIX/bin"

    cp "$cli_bin" "$INSTALL_PREFIX/bin/kylin-doctor"
    chmod +x "$INSTALL_PREFIX/bin/kylin-doctor"
    log_ok "已安装: $INSTALL_PREFIX/bin/kylin-doctor"

    if [[ -f "$web_bin" ]]; then
        cp "$web_bin" "$INSTALL_PREFIX/bin/kylin-doctor-web"
        chmod +x "$INSTALL_PREFIX/bin/kylin-doctor-web"
        log_ok "已安装: $INSTALL_PREFIX/bin/kylin-doctor-web"
    fi

    # 创建配置目录和默认配置
    local config_dir="$HOME/.kylin-doctor"
    if [[ -n "${SUDO_USER:-}" ]]; then
        config_dir=$(eval echo "~$SUDO_USER/.kylin-doctor")
    fi
    mkdir -p "$config_dir/knowledge/raw_docs"
    log_ok "配置目录: $config_dir"

    # 创建默认配置文件
    create_default_config "$config_dir"

    # 清理构建目录
    cd /
    rm -rf "$BUILD_DIR"
    log_ok "清理构建临时文件"

    log_ok "安装完成"
}

# 创建默认配置文件
create_default_config() {
    local config_dir="$1"
    local config_file="$config_dir/config.toml"

    if [[ -f "$config_file" ]]; then
        log_info "配置文件已存在: $config_file"
        return 0
    fi

    log_info "创建默认配置文件..."

    cat > "$config_file" << 'TOML'
# ================================================================
# kylin-doctor 配置文件
# ================================================================
# 位置: ~/.kylin-doctor/config.toml
# 文档: https://github.com/fanwenzhu/kylin-doctor/blob/master/docs/USAGE.md
#
# 本文件使用 TOML 格式，所有字段都有默认值。
# 只需修改你想自定义的选项，其他保持注释或删除即可。
# ================================================================

# ----------------------------------------------------------------
# 通用设置
# ----------------------------------------------------------------
[general]
# 输出详细级别: 0=简洁, 1=标准, 2=详细
# 默认值: 1
verbose = 1

# 是否自动修复发现的问题
# 默认值: false (建议手动确认后再修复)
auto_fix = false

# 自动修复前是否需要用户确认
# 默认值: true
confirm_before_fix = true

# 完全禁用网络请求 (离线模式)
# 适用场景: 内网环境、安全审计要求
# 默认值: false
offline = false

# ----------------------------------------------------------------
# AI 模型配置
# ----------------------------------------------------------------
[llm]
# AI 策略: "local" | "cloud" | "hybrid"
# - local:  使用本地 Ollama 模型 (需要先安装 Ollama)
# - cloud:  使用云端 API (需要配置 API Key)
# - hybrid: 优先本地，本地不可用时回退到云端
# 默认值: "local"
strategy = "local"

# 本地模型配置 (需要先安装 Ollama)
[llm.local]
# Ollama 服务地址
# 默认值: "http://localhost:11434"
endpoint = "http://localhost:11434"

# 对话模型名称
# 推荐配置:
#   - qwen2.5:1.5b  → 最快，适合低配机器 (4GB+ 内存)
#   - qwen2.5:3b    → 平衡速度和质量 (推荐，8GB+ 内存)
#   - qwen2.5:7b    → 最智能，需要更多内存 (16GB+ 内存)
# 默认值: "qwen2.5:3b"
model = "qwen2.5:3b"

# 云端模型配置 (需要 API Key)
[llm.cloud]
# 云服务商: "qwen" | "deepseek" | "moonshot" | "custom"
# - qwen:     通义千问 (阿里云)
# - deepseek: DeepSeek
# - moonshot: 月之暗面 (Kimi)
# - custom:   自定义 OpenAI 兼容 API
provider = "qwen"

# 云端模型名称
# qwen:     qwen-plus, qwen-turbo, qwen-max
# deepseek: deepseek-chat, deepseek-coder
# moonshot: moonshot-v1-8k, moonshot-v1-32k, moonshot-v1-128k
model = "qwen-plus"

# API Key 环境变量名
# 在 ~/.bashrc 或系统环境中设置对应的环境变量
# 例如: export QWEN_API_KEY="sk-xxxxxxxxxxxx"
api_key_env = "QWEN_API_KEY"

# API 端点 (一般不需要修改)
# qwen:     https://dashscope.aliyuncs.com/compatible-mode/v1
# deepseek: https://api.deepseek.com/v1
# moonshot: https://api.moonshot.cn/v1
endpoint = "https://dashscope.aliyuncs.com/compatible-mode/v1"

# ----------------------------------------------------------------
# Web 仪表盘配置
# ----------------------------------------------------------------
[web]
# 监听地址
# - 127.0.0.1: 仅允许本机访问 (安全)
# - 0.0.0.0:   允许远程访问 (需要配置防火墙)
# 可通过环境变量 HOST 覆盖
host = "127.0.0.1"

# 监听端口
# 可通过环境变量 PORT 覆盖
port = 8080

# ----------------------------------------------------------------
# 守护进程配置 (定时巡检)
# ----------------------------------------------------------------
[daemon]
# 巡检间隔 (秒)
# 3600 = 1小时, 86400 = 1天
interval = 3600

# 是否发送桌面通知
notify = true

# ================================================================
# 环境变量覆盖 (优先级高于本配置文件)
# ================================================================
# 以下环境变量可覆盖对应配置:
#
#   HOST=0.0.0.0          → 覆盖 web.host
#   PORT=9090             → 覆盖 web.port
#   QWEN_API_KEY=sk-xxx   → 通义千问 API Key
#   DEEPSEEK_API_KEY=xxx  → DeepSeek API Key
#   MOONSHOT_API_KEY=xxx  → Moonshot API Key
#
# 设置方式 (二选一):
#   1. 临时生效: export QWEN_API_KEY="sk-xxx"
#   2. 永久生效: 添加到 ~/.bashrc 或 /etc/environment
# ================================================================
TOML

    # 设置正确的权限
    if [[ -n "${SUDO_USER:-}" ]]; then
        chown "$SUDO_USER:$(id -gn "$SUDO_USER")" "$config_file" 2>/dev/null || true
    fi

    log_ok "配置文件已创建: $config_file"
}

step_5_install_ollama() {
    log_step 5 "配置 AI 模型 (可选)"

    if $SKIP_OLLAMA && ! $WITH_OLLAMA; then
        log_info "跳过 Ollama 安装"
        log_info "如需 AI 功能，稍后可手动安装:"
        echo "    curl -fsSL https://ollama.com/install.sh | sh"
        echo "    ollama pull qwen2.5:3b"
        echo "    ollama pull nomic-embed-text"
        return
    fi

    # 检查 zstd 依赖
    check_zstd || true

    if command -v ollama &>/dev/null; then
        log_ok "Ollama 已安装"
    else
        log_info "安装 Ollama..."
        log_to_file "下载并安装 Ollama"

        # 带重试的下载
        local retry_count=0
        local max_retries=3

        while [[ $retry_count -lt $max_retries ]]; do
            if curl -fsSL https://ollama.com/install.sh | sh >> "$LOG_FILE" 2>&1; then
                break
            fi
            retry_count=$((retry_count + 1))
            if [[ $retry_count -lt $max_retries ]]; then
                log_warn "Ollama 安装失败，重试 ($retry_count/$max_retries)..."
                sleep 2
            fi
        done

        if ! command -v ollama &>/dev/null; then
            log_warn "Ollama 安装失败，AI 功能将不可用"
            log_info "可稍后手动安装: curl -fsSL https://ollama.com/install.sh | sh"
            return
        fi
        log_ok "Ollama 安装完成"
    fi

    # 启动 Ollama 服务
    if ! pgrep -x ollama &>/dev/null; then
        log_info "启动 Ollama 服务..."
        nohup ollama serve &>/dev/null &
        sleep 3
    fi

    # 拉取对话模型
    log_info "拉取对话模型 qwen2.5:3b (约 2GB，请耐心等待)..."
    if ollama pull qwen2.5:3b >> "$LOG_FILE" 2>&1; then
        log_ok "对话模型安装完成"
    else
        log_warn "对话模型下载失败，可稍后手动执行: ollama pull qwen2.5:3b"
    fi

    # 拉取嵌入模型
    log_info "拉取嵌入模型 nomic-embed-text (约 274MB)..."
    if ollama pull nomic-embed-text >> "$LOG_FILE" 2>&1; then
        log_ok "嵌入模型安装完成"
    else
        log_warn "嵌入模型下载失败，可稍后手动执行: ollama pull nomic-embed-text"
    fi
}

step_6_verify() {
    log_step 6 "验证安装"

    local all_ok=true

    # 检查二进制
    if command -v kylin-doctor &>/dev/null; then
        local version
        version=$(kylin-doctor --version 2>/dev/null || echo "unknown")
        log_ok "kylin-doctor: $version"
    else
        log_err "kylin-doctor 未找到"
        all_ok=false
    fi

    if command -v kylin-doctor-web &>/dev/null; then
        log_ok "kylin-doctor-web: 已安装"
    else
        log_info "kylin-doctor-web: 未安装 (可选)"
    fi

    # 检查配置目录
    local config_dir="$HOME/.kylin-doctor"
    if [[ -n "${SUDO_USER:-}" ]]; then
        config_dir=$(eval echo "~$SUDO_USER/.kylin-doctor")
    fi
    if [[ -d "$config_dir" ]]; then
        log_ok "配置目录: $config_dir"
    else
        log_warn "配置目录不存在: $config_dir"
    fi

    # 检查配置文件
    local config_file="$config_dir/config.toml"
    if [[ -f "$config_file" ]]; then
        log_ok "配置文件: $config_file"
    else
        log_info "配置文件: 未创建 (使用默认配置)"
    fi

    # 检查 Ollama
    if command -v ollama &>/dev/null; then
        if pgrep -x ollama &>/dev/null; then
            log_ok "Ollama: 运行中"
        else
            log_info "Ollama: 已安装 (未运行，可用 'ollama serve' 启动)"
        fi
    else
        log_info "Ollama: 未安装 (AI 功能不可用)"
    fi

    # 快速功能测试
    if command -v kylin-doctor &>/dev/null; then
        log_info "执行快速功能测试..."
        if kylin-doctor scan --module system --quick 2>/dev/null | grep -q "扫描完成"; then
            log_ok "功能测试通过"
        else
            log_warn "功能测试未通过 (可能需要 root 权限)"
        fi
    fi

    echo ""
    if $all_ok; then
        echo -e "${GREEN}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        echo -e "${GREEN}${BOLD}  ✅ kylin-doctor 安装成功！${NC}"
        echo -e "${GREEN}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    else
        echo -e "${YELLOW}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        echo -e "${YELLOW}${BOLD}  ⚠️  安装完成，但有部分问题${NC}"
        echo -e "${YELLOW}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    fi

    echo ""
    echo "  ${BOLD}配置文件:${NC}"
    echo "    $config_file"
    echo ""
    echo "  ${BOLD}快速开始:${NC}"
    echo ""
    echo "    # 全面扫描"
    echo "    kylin-doctor scan"
    echo ""
    echo "    # 快速扫描"
    echo "    kylin-doctor scan --quick"
    echo ""
    echo "    # 启动 Web 仪表盘"
    echo "    kylin-doctor serve"
    echo "    # 浏览器打开 http://127.0.0.1:8080"
    echo ""
    echo "    # AI 对话 (需要 Ollama)"
    echo "    kylin-doctor chat"
    echo ""
    echo "    # 生成诊断报告"
    echo "    kylin-doctor report --format html --output report.html"
    echo ""
    echo "  ${BOLD}日志文件:${NC}"
    echo "    $LOG_FILE"
    echo ""
    echo "  ${BOLD}文档:${NC}"
    echo "    https://github.com/fanwenzhu/kylin-doctor/blob/master/docs/DEPLOYMENT.md"
    echo "    https://github.com/fanwenzhu/kylin-doctor/blob/master/docs/USAGE.md"
    echo ""

    log_to_file "安装完成，验证结果: all_ok=$all_ok"
}

# ============================================================
# 主流程
# ============================================================

main() {
    parse_args "$@"

    # 初始化日志
    init_log

    echo ""
    echo -e "${BOLD}${CYAN}╔══════════════════════════════════════════════════════╗${NC}"
    echo -e "${BOLD}${CYAN}║   kylin-doctor 一键安装脚本                          ║${NC}"
    echo -e "${BOLD}${CYAN}║   银河麒麟桌面系统自我诊断工具                       ║${NC}"
    echo -e "${BOLD}${CYAN}╚══════════════════════════════════════════════════════╝${NC}"
    echo ""

    check_root "$@"
    step_1_check_environment

    # 依赖冲突修复
    if $FIX_DEPS; then
        log_step "2" "修复依赖冲突"
        fix_libssl_dev
    fi

    step_2_install_deps
    step_3_install_rust
    step_4_build_install
    step_5_install_ollama
    step_6_verify
}

main "$@"
