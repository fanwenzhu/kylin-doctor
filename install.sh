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
#   --prefix <path>   安装目录 (默认: /usr/local)
#   --branch <name>   Git 分支 (默认: master)
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

# ============================================================
# 辅助函数
# ============================================================

log_info()  { echo -e "  ${INFO} $*"; }
log_ok()    { echo -e "  ${OK} $*"; }
log_warn()  { echo -e "  ${WARN} $*"; }
log_err()   { echo -e "  ${ERR} $*"; }
log_step()  { echo -e "\n${BOLD}${CYAN}[$1/6]${NC} ${BOLD}$*${NC}"; }

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
  --prefix <path>   安装目录 (默认: /usr/local)
  --branch <name>   Git 分支 (默认: master)
  --help            显示此帮助信息

示例:
  # 基础安装
  sudo ./install.sh

  # 安装并配置 AI 模型
  sudo ./install.sh --with-ollama

  # 自定义安装目录
  sudo ./install.sh --prefix /opt/kylin-doctor

  # 跳过依赖 (已安装过)
  sudo ./install.sh --skip-deps
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --skip-deps)    SKIP_DEPS=true ;;
            --skip-rust)    SKIP_RUST=true ;;
            --skip-ollama)  SKIP_OLLAMA=true ;;
            --with-ollama)  WITH_OLLAMA=true; SKIP_OLLAMA=false ;;
            --prefix)
                shift
                INSTALL_PREFIX="${1:-/usr/local}"
                ;;
            --branch)
                shift
                BRANCH="${1:-master}"
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
        $PKG_UPDATE &>/dev/null || true
        $PKG_INSTALL "${missing[@]}" &>/dev/null || {
            log_err "安装失败，请手动安装: ${missing[*]}"
            exit 1
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
    $PKG_UPDATE &>/dev/null || true

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
        $PKG_INSTALL "${to_install[@]}" 2>/dev/null || {
            log_warn "部分包安装失败，继续安装..."
        }
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
            rustup update stable 2>/dev/null || {
                log_err "Rust 更新失败"
                exit 1
            }
        fi
    else
        log_info "安装 Rust 工具链..."
        # 设置非交互模式
        export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.cargo}"
        export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"

        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
            sh -s -- -y --default-toolchain stable 2>&1 | tail -3

        if [[ -f "$CARGO_HOME/env" ]]; then
            source "$CARGO_HOME/env"
        elif [[ -f "$HOME/.cargo/env" ]]; then
            source "$HOME/.cargo/env"
        fi

        if ! command -v rustc &>/dev/null; then
            log_err "Rust 安装失败"
            exit 1
        fi

        log_ok "Rust 安装完成 ($(rustc --version))"
    fi

    # 确保 cargo 在 PATH 中
    export PATH="$HOME/.cargo/bin:$PATH"
}

step_4_build_install() {
    log_step 4 "编译并安装 kylin-doctor"

    # 清理旧的构建目录
    rm -rf "$BUILD_DIR"

    log_info "克隆仓库..."
    git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$BUILD_DIR" 2>&1 | tail -1

    cd "$BUILD_DIR"

    log_info "编译项目 (release 模式，可能需要几分钟)..."
    echo ""
    cargo build --release 2>&1 | grep -E "Compiling|Finished" || true
    echo ""

    # 检查编译产物
    local bin_dir="$BUILD_DIR/target/release"
    local cli_bin="$bin_dir/kylin-doctor"
    local web_bin="$bin_dir/kylin-doctor-web"

    if [[ ! -f "$cli_bin" ]]; then
        log_err "CLI 二进制编译失败"
        exit 1
    fi
    log_ok "CLI 编译成功"

    if [[ ! -f "$web_bin" ]]; then
        log_warn "Web 二进制编译失败 (非致命)"
    else
        log_ok "Web 二进制编译成功"
    fi

    # 运行测试
    log_info "运行测试..."
    if cargo test --quiet 2>/dev/null; then
        log_ok "所有测试通过"
    else
        log_warn "部分测试失败 (非致命，继续安装)"
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

    # 创建配置目录
    local config_dir="$HOME/.kylin-doctor"
    if [[ -n "${SUDO_USER:-}" ]]; then
        config_dir=$(eval echo "~$SUDO_USER/.kylin-doctor")
    fi
    mkdir -p "$config_dir/knowledge/raw_docs"
    log_ok "配置目录: $config_dir"

    # 清理构建目录
    cd /
    rm -rf "$BUILD_DIR"
    log_ok "清理构建临时文件"

    log_ok "安装完成"
}

step_5_install_ollama() {
    log_step 5 "配置 AI 模型 (可选)"

    if $SKIP_OLLAMA && ! $WITH_OLLAMA; then
        log_info "跳过 Ollama 安装"
        log_info "如需 AI 功能，稍后可手动安装:"
        echo "    curl -fsSL https://ollama.com/install.sh | sh"
        echo "    ollama pull qwen2.5:7b"
        echo "    ollama pull nomic-embed-text"
        return
    fi

    if command -v ollama &>/dev/null; then
        log_ok "Ollama 已安装"
    else
        log_info "安装 Ollama..."
        curl -fsSL https://ollama.com/install.sh | sh 2>&1 | tail -3

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
    log_info "拉取对话模型 qwen2.5:7b (约 4.7GB，请耐心等待)..."
    if ollama pull qwen2.5:7b 2>&1 | tail -3; then
        log_ok "对话模型安装完成"
    else
        log_warn "对话模型下载失败，可稍后手动执行: ollama pull qwen2.5:7b"
    fi

    # 拉取嵌入模型
    log_info "拉取嵌入模型 nomic-embed-text (约 274MB)..."
    if ollama pull nomic-embed-text 2>&1 | tail -3; then
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
        echo -e "${GREEN}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        echo -e "${GREEN}${BOLD}  ✅ kylin-doctor 安装成功！${NC}"
        echo -e "${GREEN}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    else
        echo -e "${YELLOW}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        echo -e "${YELLOW}${BOLD}  ⚠️  安装完成，但有部分问题${NC}"
        echo -e "${YELLOW}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    fi

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
    echo "  ${BOLD}文档:${NC}"
    echo "    https://github.com/fanwenzhu/kylin-doctor/blob/master/docs/DEPLOYMENT.md"
    echo "    https://github.com/fanwenzhu/kylin-doctor/blob/master/docs/USAGE.md"
    echo ""
}

# ============================================================
# 主流程
# ============================================================

main() {
    parse_args "$@"

    echo ""
    echo -e "${BOLD}${CYAN}╔══════════════════════════════════════════╗${NC}"
    echo -e "${BOLD}${CYAN}║   kylin-doctor 一键安装脚本              ║${NC}"
    echo -e "${BOLD}${CYAN}║   银河麒麟桌面系统自我诊断工具           ║${NC}"
    echo -e "${BOLD}${CYAN}╚══════════════════════════════════════════╝${NC}"
    echo ""

    check_root "$@"
    step_1_check_environment
    step_2_install_deps
    step_3_install_rust
    step_4_build_install
    step_5_install_ollama
    step_6_verify
}

main "$@"
