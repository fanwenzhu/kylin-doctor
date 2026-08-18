#!/bin/bash
set -euo pipefail

# ============================================================
# kylin-doctor deb 打包脚本
# 用法: ./build-deb.sh [--arch amd64|arm64|loongarch64] [--skip-build] [--static]
# 注: amd64/arm64 默认 gnu 动态，--static 切 musl 静态（发布用）；
#     loongarch64 仅支持本机编译（交叉编译结构性不可行，见下方说明），
#     在非龙芯宿主上 --arch loongarch64 会报错并引导用 install.sh。
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

# 构建中途失败时清理半成品 BUILD_DIR（成功时已由脚本末尾删除，trap 再删为空操作）
BUILD_DIR=""
trap 'rm -rf "$BUILD_DIR" 2>/dev/null || true' EXIT

# --- 颜色 ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log_info()  { echo -e "${CYAN}[INFO]${NC} $*"; }
log_ok()    { echo -e "${GREEN}[OK]${NC} $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_err()   { echo -e "${RED}[ERROR]${NC} $*"; }

# --- 参数解析 ---
ARCH=""
SKIP_BUILD=false
STATIC=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --arch)     ARCH="$2"; shift 2 ;;
        --skip-build) SKIP_BUILD=true; shift ;;
        --static)   STATIC=true; shift ;;
        -h|--help)
            echo "用法: $0 [--arch amd64|arm64|loongarch64] [--skip-build] [--static]"
            echo ""
            echo "选项:"
            echo "  --arch ARCH      指定目标架构 (amd64、arm64 或 loongarch64，默认自动检测)"
            echo "  --skip-build     跳过编译，使用已有的二进制"
            echo "  --static         使用 musl 静态编译，消除 glibc 依赖（仅 amd64/arm64）"
            echo "  -h, --help       显示帮助"
            echo ""
            echo "示例:"
            echo "  $0                          # 编译当前架构并打包"
            echo "  $0 --static                 # 静态编译当前架构并打包（amd64/arm64 musl）"
            echo "  $0 --arch arm64             # 交叉编译 arm64 并打包"
            echo "  $0 --arch loongarch64       # ⚠️ 龙芯不支持交叉编译，请在龙芯本机跑 install.sh"
            echo "  $0 --arch amd64 --skip-build  # 跳过编译，直接打包"
            exit 0
            ;;
        *) log_err "未知参数: $1"; exit 1 ;;
    esac
done

# --- 检测架构 ---
HOST_ARCH=$(uname -m)
if [[ -z "$ARCH" ]]; then
    case "$HOST_ARCH" in
        x86_64)  ARCH="amd64" ;;
        aarch64) ARCH="arm64" ;;
        loongarch64) ARCH="loongarch64" ;;
        *) log_err "不支持的架构: $HOST_ARCH"; exit 1 ;;
    esac
fi

# loongarch64 (龙芯 LoongArch) 不支持交叉编译打 deb，只能本机编译：
#   - musl 静态（zig）：loongarch64-linux-musl 的 signal/sigaction 实现有缺陷，
#     Rust std 启动时 signal(SIGPIPE, SIG_IGN) 返回 SIG_ERR 触发断言 abort。
#   - gnu 动态（交叉）：构建机 glibc≥2.36 编出的二进制引用 GLIBC_2.36 符号，
#     而麒麟 loongarch 把 loongarch backport 到 glibc 2.28，运行时 GLIBC_2.36
#     not found 崩溃（ABI 不兼容，无法靠 patchelf/改依赖绕过）。
# 故 loongarch64 只能在龙芯本机用本机 glibc 编译（产物符号与本机匹配）。
# 交叉编译到 loongarch64 直接报错，引导用户用 install.sh 本机编译。
if [[ "$ARCH" == "loongarch64" && "$HOST_ARCH" != "loongarch64" ]]; then
    log_err "loongarch64 不支持交叉编译打 deb（musl 撞 SIGPIPE bug，gnu 撞 glibc 符号不匹配）"
    log_err "请在龙芯本机用 install.sh 编译安装："
    log_err "  curl -fsSL https://raw.githubusercontent.com/fanwenzhu/kylin-doctor/master/install.sh | sudo bash"
    exit 1
fi
# loongarch64 本机编译：不支持 --static（无 loongarch musl 工具链），强制 gnu 动态本机编
if [[ "$ARCH" == "loongarch64" && "$STATIC" == true ]]; then
    log_warn "loongarch64 无 musl 工具链，改用 gnu 动态本机编译"
    STATIC=false
fi

# 判断是否需要交叉编译
CROSS=false
CARGO_TARGET=""
if [[ "$STATIC" == true ]]; then
    case "$ARCH" in
        amd64)   CARGO_TARGET="x86_64-unknown-linux-musl" ;;
        arm64)   CARGO_TARGET="aarch64-unknown-linux-musl" ;;
        *) log_err "不支持的目标架构: $ARCH（musl 静态仅支持 amd64/arm64）"; exit 1 ;;
    esac
else
    case "$ARCH" in
        amd64)        CARGO_TARGET="x86_64-unknown-linux-gnu" ;;
        arm64)        CARGO_TARGET="aarch64-unknown-linux-gnu" ;;
        loongarch64)  CARGO_TARGET="loongarch64-unknown-linux-gnu" ;;  # 仅本机编译可达
        *) log_err "不支持的目标架构: $ARCH"; exit 1 ;;
    esac
fi

# 宿主与目标架构不同即为交叉编译
HOST_TARGET=""
case "$HOST_ARCH" in
    x86_64)       HOST_TARGET="amd64" ;;
    aarch64)      HOST_TARGET="arm64" ;;
    loongarch64)  HOST_TARGET="loongarch64" ;;
    *) log_err "不支持的宿主架构: $HOST_ARCH"; exit 1 ;;
esac
if [[ -n "$HOST_TARGET" && "$ARCH" != "$HOST_TARGET" ]]; then
    CROSS=true
fi

log_info "宿主架构: $HOST_ARCH"
log_info "目标架构: $ARCH"
[[ "$STATIC" == true ]] && log_info "静态编译: $CARGO_TARGET (musl)"
[[ "$CROSS" == true ]] && log_info "交叉编译: $CARGO_TARGET"

# --- 读取版本号 ---
if ! grep -q '^version' Cargo.toml; then
    log_err "无法从 Cargo.toml 读取版本号（未找到 version 字段）"
    exit 1
fi
VERSION=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')

PACKAGE_NAME="kylin-doctor"
DEB_NAME="${PACKAGE_NAME}_${VERSION}_${ARCH}"
DIST_DIR="$SCRIPT_DIR/dist"
BUILD_DIR="$DIST_DIR/$DEB_NAME"

log_info "版本: $VERSION"
log_info "输出: $DIST_DIR/${DEB_NAME}.deb"

# --- 编译 ---
if [[ "$SKIP_BUILD" == false ]]; then
    if [[ "$STATIC" == true ]]; then
        log_info "静态编译 release 版本 ($CARGO_TARGET)..."

        export RUSTFLAGS='-C target-feature=+crt-static'

        # 判断是否需要交叉编译
        if [[ "$HOST_ARCH" == "x86_64" && "$ARCH" == "arm64" ]]; then
            # x86_64 -> aarch64 musl 交叉编译，使用 cross
            if ! which cross >/dev/null 2>&1; then
                log_err "找不到 cross 工具"
                log_err "安装: cargo install cross"
                exit 1
            fi
            log_info "使用 cross 进行 aarch64 musl 交叉编译..."
            cross build --release --target "$CARGO_TARGET" 2>&1
        else
            # 本机 musl 编译，检查 musl-gcc
            if ! which musl-gcc >/dev/null 2>&1; then
                log_err "找不到 musl-gcc"
                log_err "安装: sudo apt install musl-tools (Debian) 或 sudo dnf install musl-devel (RHEL)"
                log_err "或从源码编译: https://musl.libc.org/"
                exit 1
            fi
            cargo build --release --target "$CARGO_TARGET" 2>&1
        fi
    elif [[ "$CROSS" == true ]]; then
        log_info "交叉编译 release 版本 ($CARGO_TARGET)..."

        # 检查交叉编译器（CARGO_TARGET_*_LINKER 是 cargo/rustc 的 -C linker 约定；
        # CC_<target> 是 cc crate 识别的形式，编译 ring 的 C/汇编用）
        case "$ARCH" in
            arm64)
                if ! which aarch64-linux-gnu-gcc >/dev/null 2>&1; then
                    log_err "找不到交叉编译器 aarch64-linux-gnu-gcc"
                    log_err "安装: sudo apt install gcc-aarch64-linux-gnu (Debian) 或 sudo dnf install gcc-aarch64-linux-gnu (RHEL)"
                    exit 1
                fi
                export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
                export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
                ;;
            amd64)
                if ! which x86_64-linux-gnu-gcc >/dev/null 2>&1; then
                    log_err "找不到交叉编译器 x86_64-linux-gnu-gcc"
                    log_err "安装: sudo apt install gcc-x86-64-linux-gnu (Debian) 或 sudo dnf install gcc-x86-64-linux-gnu (RHEL)"
                    exit 1
                fi
                export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc
                export CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc
                ;;
            *) log_err "未配置 $ARCH 的 gnu 交叉编译器支持"; exit 1 ;;
        esac

        # 确保 rust-std 目标已安装（cargo 交叉编译需要）
        if ! rustup target list --installed 2>/dev/null | grep -q "$CARGO_TARGET"; then
            log_info "安装 rust-std 目标 $CARGO_TARGET..."
            rustup target add "$CARGO_TARGET" || { log_err "rustup target add 失败"; exit 1; }
        fi

        cargo build --release --target "$CARGO_TARGET" 2>&1
    else
        log_info "编译 release 版本..."
        cargo build --release 2>&1
    fi
    log_ok "编译完成"
else
    log_warn "跳过编译，使用已有二进制"
fi

# --- 确定二进制路径 ---
if [[ "$CROSS" == true ]] || [[ "$STATIC" == true ]]; then
    BIN_DIR="$SCRIPT_DIR/target/$CARGO_TARGET/release"
else
    BIN_DIR="$SCRIPT_DIR/target/release"
fi

BIN_CLI="$BIN_DIR/kylin-doctor"
BIN_WEB="$BIN_DIR/kylin-doctor-web"

if [[ ! -f "$BIN_CLI" ]]; then
    log_err "找不到二进制: $BIN_CLI"
    exit 1
fi
if [[ ! -f "$BIN_WEB" ]]; then
    log_err "找不到二进制: $BIN_WEB"
    exit 1
fi

# --- 清理并创建目录结构 ---
log_info "创建 deb 目录结构..."
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR/DEBIAN"
mkdir -p "$BUILD_DIR/usr/bin"
mkdir -p "$BUILD_DIR/usr/lib/systemd/system"
mkdir -p "$BUILD_DIR/usr/share/$PACKAGE_NAME"
mkdir -p "$BUILD_DIR/usr/share/doc/$PACKAGE_NAME"

# --- 复制文件 ---
log_info "复制文件..."

# 二进制
cp "$BIN_CLI" "$BUILD_DIR/usr/bin/kylin-doctor"
cp "$BIN_WEB" "$BUILD_DIR/usr/bin/kylin-doctor-web"
chmod 755 "$BUILD_DIR/usr/bin/kylin-doctor"
chmod 755 "$BUILD_DIR/usr/bin/kylin-doctor-web"

# systemd 服务
cp "$SCRIPT_DIR/pkg/deb/kylin-doctor-web.service" \
   "$BUILD_DIR/usr/lib/systemd/system/kylin-doctor-web.service"

# 配置模板
cp "$SCRIPT_DIR/pkg/deb/config.toml.example" \
   "$BUILD_DIR/usr/share/$PACKAGE_NAME/config.toml.example"

# 文档
cp "$SCRIPT_DIR/README.md" "$BUILD_DIR/usr/share/doc/$PACKAGE_NAME/"
cp "$SCRIPT_DIR/CHANGELOG.md" "$BUILD_DIR/usr/share/doc/$PACKAGE_NAME/"
cp "$SCRIPT_DIR/docs/DEPLOYMENT.md" "$BUILD_DIR/usr/share/doc/$PACKAGE_NAME/"
cp "$SCRIPT_DIR/docs/USAGE.md" "$BUILD_DIR/usr/share/doc/$PACKAGE_NAME/"

# --- 生成 DEBIAN/control ---
log_info "生成 control 文件..."
cat > "$BUILD_DIR/DEBIAN/control" << EOF
Package: $PACKAGE_NAME
Version: $VERSION
Architecture: $ARCH
Maintainer: fanwenzhu <fanwenzhu@github.com>
Installed-Size: $(du -sk "$BUILD_DIR" | cut -f1)
Recommends: procps, coreutils, pciutils, usbutils, smartmontools, dmidecode, lm-sensors, iproute2, iputils-ping, fontconfig, ca-certificates
Section: utils
Priority: optional
Homepage: https://github.com/fanwenzhu/kylin-doctor
Description: 银河麒麟桌面系统自我诊断工具
 硬件、系统、软件、安全、性能五大维度全面诊断。
 支持 AI 智能分析（本地 Ollama + 云端大模型）。
 提供 CLI 命令行和 Web 仪表盘两种使用方式。
EOF

# loongarch64 本机编译的 gnu 动态包依赖系统 glibc。阈值取已知最低运行环境
# （麒麟 V10 loongarch glibc 2.28）；本机编译产物符号≤本机 glibc，声明值仅作
# 下限提示。amd64/arm64 走 musl 静态无 C 库依赖，不声明 Depends。
if [[ "$ARCH" == "loongarch64" && "$STATIC" == false ]]; then
    sed -i '/^Recommends:/i Depends: libc6 (>= 2.28)' "$BUILD_DIR/DEBIAN/control"
fi

# --- 生成 DEBIAN/postinst ---
log_info "生成 postinst 脚本..."
cat > "$BUILD_DIR/DEBIAN/postinst" << 'POSTINST'
#!/bin/bash
set -e

echo ""
echo "  ✅ kylin-doctor 安装成功！"
echo ""
echo "  快速开始:"
echo "    kylin-doctor scan              # 全面扫描"
echo "    kylin-doctor scan --quick      # 快速扫描"
echo "    kylin-doctor chat              # AI 对话"
echo ""
echo "  Web 仪表盘:"
echo "    systemctl enable --now kylin-doctor-web   # 启用服务"
echo "    浏览器打开 http://127.0.0.1:8080"
echo ""
echo "  配置文件:"
echo "    cp /usr/share/kylin-doctor/config.toml.example ~/.kylin-doctor/config.toml"
echo "    vim ~/.kylin-doctor/config.toml"
echo ""

# 如果 systemd 可用，重载 daemon
if command -v systemctl >/dev/null 2>&1; then
    systemctl daemon-reload 2>/dev/null || true
fi

# 配置文件首次创建
CONFIG_DIR="$HOME/.kylin-doctor"
if [[ ! -d "$CONFIG_DIR" ]]; then
    mkdir -p "$CONFIG_DIR/knowledge/raw_docs"
    cp /usr/share/kylin-doctor/config.toml.example "$CONFIG_DIR/config.toml"
    echo "  已创建默认配置: $CONFIG_DIR/config.toml"
fi

# 如果通过 sudo 安装，为实际用户设置权限
if [[ -n "${SUDO_USER:-}" ]]; then
    chown -R "$SUDO_USER:$(id -gn "$SUDO_USER")" "$CONFIG_DIR" 2>/dev/null || true
fi
POSTINST
chmod 755 "$BUILD_DIR/DEBIAN/postinst"

# --- 生成 DEBIAN/prerm ---
log_info "生成 prerm 脚本..."
cat > "$BUILD_DIR/DEBIAN/prerm" << 'PRERM'
#!/bin/bash
set -e

# 停止 Web 服务
if command -v systemctl >/dev/null 2>&1 && systemctl is-active --quiet kylin-doctor-web 2>/dev/null; then
    echo "停止 kylin-doctor-web 服务..."
    systemctl stop kylin-doctor-web 2>/dev/null || true
fi

# 停止可能的进程
pkill -f kylin-doctor-web 2>/dev/null || true
PRERM
chmod 755 "$BUILD_DIR/DEBIAN/prerm"

# --- 生成 DEBIAN/postrm ---
log_info "生成 postrm 脚本..."
cat > "$BUILD_DIR/DEBIAN/postrm" << 'POSTRM'
#!/bin/bash
set -e

if [ "$1" = "purge" ] || [ "$1" = "remove" ]; then
    # 重载 systemd
    if command -v systemctl >/dev/null 2>&1; then
        systemctl daemon-reload 2>/dev/null || true
    fi
fi
POSTRM
chmod 755 "$BUILD_DIR/DEBIAN/postrm"

# --- 构建 deb ---
log_info "构建 deb 包..."
mkdir -p "$DIST_DIR"
dpkg-deb --build --root-owner-group "$BUILD_DIR" "$DIST_DIR/${DEB_NAME}.deb"

# --- 清理构建目录 ---
rm -rf "$BUILD_DIR"

# --- 完成 ---
DEB_FILE="$DIST_DIR/${DEB_NAME}.deb"
DEB_SIZE=$(du -h "$DEB_FILE" | cut -f1)

echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}✅ 打包完成${NC}"
echo ""
echo -e "  文件: ${BOLD}$DEB_FILE${NC}"
echo -e "  大小: $DEB_SIZE"
echo ""
echo "  安装: sudo dpkg -i $DEB_FILE"
echo "  卸载: sudo dpkg -r $PACKAGE_NAME"
echo ""
