#!/usr/bin/env bash
#
# kylin-doctor 卸载脚本
#
# 用法:
#   sudo ./uninstall.sh [选项]
#
# 选项:
#   --keep-config    保留配置文件和知识库数据
#   --remove-ollama  同时卸载 Ollama
#   --help           显示帮助信息

set -euo pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

OK="${GREEN}✓${NC}"
WARN="${YELLOW}⚠${NC}"
INFO="${CYAN}ℹ${NC}"

KEEP_CONFIG=false
REMOVE_OLLAMA=false
INSTALL_PREFIX="/usr/local"

show_help() {
    cat << 'EOF'
kylin-doctor 卸载脚本

用法: sudo ./uninstall.sh [选项]

选项:
  --keep-config     保留 ~/.kylin-doctor 配置和知识库数据
  --remove-ollama   同时卸载 Ollama 及其模型
  --prefix <path>   安装目录 (默认: /usr/local)
  --help            显示此帮助信息
EOF
}

# 解析参数
while [[ $# -gt 0 ]]; do
    case "$1" in
        --keep-config)   KEEP_CONFIG=true ;;
        --remove-ollama) REMOVE_OLLAMA=true ;;
        --prefix)        shift; INSTALL_PREFIX="${1:-/usr/local}" ;;
        --help|-h)       show_help; exit 0 ;;
        *)               echo "未知选项: $1"; show_help; exit 1 ;;
    esac
    shift
done

# 检查 root
if [[ $EUID -ne 0 ]]; then
    echo -e "${RED}✗ 此脚本需要 root 权限${NC}"
    echo "  请使用: sudo $0 $*"
    exit 1
fi

echo ""
echo -e "${BOLD}${CYAN}╔══════════════════════════════════════════╗${NC}"
echo -e "${BOLD}${CYAN}║   kylin-doctor 卸载程序                  ║${NC}"
echo -e "${BOLD}${CYAN}╚══════════════════════════════════════════╝${NC}"
echo ""

# 1. 停止运行中的服务
echo -e "${BOLD}[1/4] 停止服务${NC}"

if pgrep -x kylin-doctor-web &>/dev/null; then
    pkill kylin-doctor-web 2>/dev/null || true
    echo -e "  ${OK} 已停止 kylin-doctor-web"
else
    echo -e "  ${INFO} kylin-doctor-web 未运行"
fi

if systemctl is-active --quiet kylin-doctor-web 2>/dev/null; then
    systemctl stop kylin-doctor-web 2>/dev/null || true
    systemctl disable kylin-doctor-web 2>/dev/null || true
    echo -e "  ${OK} 已停止 systemd 服务"
fi

# 2. 删除二进制文件
echo -e "\n${BOLD}[2/4] 删除程序文件${NC}"

for bin in kylin-doctor kylin-doctor-web; do
    local_bin="$INSTALL_PREFIX/bin/$bin"
    if [[ -f "$local_bin" ]]; then
        rm -f "$local_bin"
        echo -e "  ${OK} 已删除: $local_bin"
    else
        echo -e "  ${INFO} 未找到: $local_bin"
    fi
done

# 删除 systemd 服务文件
if [[ -f /etc/systemd/system/kylin-doctor-web.service ]]; then
    rm -f /etc/systemd/system/kylin-doctor-web.service
    systemctl daemon-reload 2>/dev/null || true
    echo -e "  ${OK} 已删除 systemd 服务"
fi

# 3. 处理配置文件
echo -e "\n${BOLD}[3/4] 处理配置文件${NC}"

config_dir="$HOME/.kylin-doctor"
if [[ -n "${SUDO_USER:-}" ]]; then
    config_dir=$(eval echo "~$SUDO_USER/.kylin-doctor")
fi

if [[ -d "$config_dir" ]]; then
    if $KEEP_CONFIG; then
        echo -e "  ${INFO} 保留配置目录: $config_dir (--keep-config)"
    else
        rm -rf "$config_dir"
        echo -e "  ${OK} 已删除: $config_dir"
    fi
else
    echo -e "  ${INFO} 配置目录不存在"
fi

# 4. 可选: 卸载 Ollama
echo -e "\n${BOLD}[4/4] Ollama 清理${NC}"

if $REMOVE_OLLAMA; then
    # 停止 Ollama
    if pgrep -x ollama &>/dev/null; then
        pkill ollama 2>/dev/null || true
        sleep 2
    fi

    # 删除 Ollama 二进制
    if [[ -f /usr/local/bin/ollama ]]; then
        rm -f /usr/local/bin/ollama
        echo -e "  ${OK} 已删除: /usr/local/bin/ollama"
    fi

    # 删除 Ollama 数据
    if [[ -d /usr/share/ollama ]]; then
        rm -rf /usr/share/ollama
        echo -e "  ${OK} 已删除: /usr/share/ollama"
    fi

    # 删除 Ollama 用户
    if id ollama &>/dev/null 2>&1; then
        userdel ollama 2>/dev/null || true
        echo -e "  ${OK} 已删除 ollama 用户"
    fi

    # 删除 systemd 服务
    if [[ -f /etc/systemd/system/ollama.service ]]; then
        systemctl stop ollama 2>/dev/null || true
        systemctl disable ollama 2>/dev/null || true
        rm -f /etc/systemd/system/ollama.service
        systemctl daemon-reload 2>/dev/null || true
        echo -e "  ${OK} 已删除 Ollama 服务"
    fi

    echo -e "  ${OK} Ollama 已完全卸载"
else
    if command -v ollama &>/dev/null; then
        echo -e "  ${INFO} Ollama 保留安装 (如需卸载请使用 --remove-ollama)"
    else
        echo -e "  ${INFO} Ollama 未安装"
    fi
fi

# 完成
echo ""
echo -e "${GREEN}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}${BOLD}  ✅ kylin-doctor 已卸载${NC}"
echo -e "${GREEN}${BOLD}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "  如需重新安装:"
echo "    curl -fsSL https://raw.githubusercontent.com/fanwenzhu/kylin-doctor/master/install.sh | sudo bash"
echo ""
