#!/bin/bash
set -euo pipefail

# ============================================================
# kylin-doctor deb 打包脚本
# 用法: ./build-deb.sh [--arch amd64|arm64] [--skip-build]
# ============================================================

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

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

while [[ $# -gt 0 ]]; do
    case "$1" in
        --arch)     ARCH="$2"; shift 2 ;;
        --skip-build) SKIP_BUILD=true; shift ;;
        -h|--help)
            echo "用法: $0 [--arch amd64|arm64] [--skip-build]"
            echo ""
            echo "选项:"
            echo "  --arch ARCH      指定目标架构 (amd64 或 arm64，默认自动检测)"
            echo "  --skip-build     跳过编译，使用已有的 target/release 二进制"
            echo "  -h, --help       显示帮助"
            exit 0
            ;;
        *) log_err "未知参数: $1"; exit 1 ;;
    esac
done

# --- 检测架构 ---
if [[ -z "$ARCH" ]]; then
    case "$(uname -m)" in
        x86_64)  ARCH="amd64" ;;
        aarch64) ARCH="arm64" ;;
        *) log_err "不支持的架构: $(uname -m)"; exit 1 ;;
    esac
fi

log_info "目标架构: $ARCH"

# --- 读取版本号 ---
VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
if [[ -z "$VERSION" ]]; then
    log_err "无法从 Cargo.toml 读取版本号"
    exit 1
fi

PACKAGE_NAME="kylin-doctor"
DEB_NAME="${PACKAGE_NAME}_${VERSION}_${ARCH}"
DIST_DIR="$SCRIPT_DIR/dist"
BUILD_DIR="$DIST_DIR/$DEB_NAME"

log_info "版本: $VERSION"
log_info "输出: $DIST_DIR/${DEB_NAME}.deb"

# --- 编译 ---
if [[ "$SKIP_BUILD" == false ]]; then
    log_info "编译 release 版本..."
    cargo build --release 2>&1
    log_ok "编译完成"
else
    log_warn "跳过编译，使用已有二进制"
fi

# --- 检查二进制文件 ---
BIN_CLI="$SCRIPT_DIR/target/release/kylin-doctor"
BIN_WEB="$SCRIPT_DIR/target/release/kylin-doctor-web"

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
Recommends: procps, coreutils, pciutils, usbutils, smartmontools, dmidecode, lm-sensors, iproute2, iputils-ping, fontconfig
Section: utils
Priority: optional
Homepage: https://github.com/fanwenzhu/kylin-doctor
Description: 银河麒麟桌面系统自我诊断工具
 硬件、系统、软件、安全、性能五大维度全面诊断。
 支持 AI 智能分析（本地 Ollama + 云端大模型）。
 提供 CLI 命令行和 Web 仪表盘两种使用方式。
EOF

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
