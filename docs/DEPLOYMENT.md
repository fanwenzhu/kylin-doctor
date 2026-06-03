# kylin-doctor 部署文档

> 银河麒麟桌面系统自我诊断工具 — 部署与运维指南

---

## 目录

- [环境要求](#环境要求)
- [从源码构建](#从源码构建)
- [安装与配置](#安装与配置)
- [CLI 部署](#cli-部署)
- [Web 仪表盘部署](#web-仪表盘部署)
- [AI 模型配置](#ai-模型配置)
- [知识库配置](#知识库配置)
- [系统服务部署](#系统服务部署)
- [Docker 部署](#docker-部署)
- [故障排查](#故障排查)

---

## 环境要求

### 操作系统

| 平台 | 版本 | 支持状态 |
|------|------|----------|
| 银河麒麟桌面版 V10 | SP1+ | ✅ 完全支持 |
| 银河麒麟桌面版 V11 | - | ✅ 完全支持 |
| Ubuntu/Debian 系 | 20.04+ | ✅ 基本支持 |
| 其他 Linux | - | ⚠️ 部分功能可能受限 |

### 硬件最低要求

| 资源 | 最低要求 | 推荐配置 |
|------|----------|----------|
| CPU | 1 核 | 2 核+ |
| 内存 | 512 MB | 2 GB+ |
| 磁盘 | 100 MB（程序） + 500 MB（模型，可选） | 2 GB+ |
| 网络 | 无要求（离线可用） | 用于云端 AI 和软件源检查 |

### 依赖工具

以下工具被各检测模块使用，建议全部安装以获得完整诊断能力：

```bash
# 核心依赖（必须）
sudo apt install -y procps coreutils

# 系统检测
sudo apt install -y systemd

# 硬件检测
sudo apt install -y smartmontools pciutils usbutils dmidecode lm-sensors

# 软件检测
sudo apt install -y dpkg apt fontconfig snapd flatpak

# 安全检测
sudo apt install -y openssh-server ufw auditd

# 性能检测
sudo apt install -y iproute2 iputils-ping

# 编译依赖（仅构建时需要）
sudo apt install -y build-essential pkg-config libssl-dev
```

> 💡 缺少某些工具不会导致程序崩溃，相关检测项会自动跳过并报告为"不可用"。

---

## 一键安装（推荐）

```bash
# 基础安装
curl -fsSL https://raw.githubusercontent.com/fanwenzhu/kylin-doctor/master/install.sh | sudo bash

# 安装并自动配置 AI 模型
curl -fsSL https://raw.githubusercontent.com/fanwenzhu/kylin-doctor/master/install.sh | sudo bash -s -- --with-ollama

# 查看所有选项
curl -fsSL https://raw.githubusercontent.com/fanwenzhu/kylin-doctor/master/install.sh | bash -s -- --help
```

安装脚本会自动完成：检测系统环境 → 安装依赖 → 安装 Rust → 编译安装 → 配置 Ollama（可选）→ 验证安装。

```bash
# 卸载
curl -fsSL https://raw.githubusercontent.com/fanwenzhu/kylin-doctor/master/uninstall.sh | sudo bash
```

---

## 从源码构建

### 1. 安装 Rust 工具链

```bash
# 安装 rustup（官方安装器）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# 加载环境变量
source "$HOME/.cargo/env"

# 验证安装
rustc --version    # 需要 1.70+
cargo --version
```

### 2. 克隆仓库

```bash
git clone https://github.com/fanwenzhu/kylin-doctor.git
cd kylin-doctor
```

### 3. 构建

```bash
# 开发构建（编译快，二进制较大）
cargo build

# 发布构建（编译慢，二进制优化）
cargo build --release

# 构建产物位置
ls -lh target/release/kylin-doctor       # CLI 工具
ls -lh target/release/kylin-doctor-web   # Web 仪表盘（独立二进制）
```

### 4. 运行测试

```bash
# 运行全部测试（52 个单元测试）
cargo test

# 只运行核心库测试
cargo test -p kylin-doctor-core

# 检查编译警告
cargo check
```

### 5. 安装到系统

```bash
# 方式一：直接复制
sudo cp target/release/kylin-doctor /usr/local/bin/
sudo cp target/release/kylin-doctor-web /usr/local/bin/

# 方式二：cargo install（从项目目录）
cargo install --path crates/kylin-doctor-cli
cargo install --path crates/kylin-doctor-web

# 验证安装
kylin-doctor --version
kylin-doctor-web --version
```

---

## 安装与配置

### 配置文件

首次运行时，kylin-doctor 会使用默认配置。如需自定义，创建配置文件：

```bash
mkdir -p ~/.kylin-doctor
```

创建 `~/.kylin-doctor/config.toml`：

```toml
# kylin-doctor 配置文件
# 位置: ~/.kylin-doctor/config.toml

[general]
verbose = 1              # 输出详细程度: 0=简要, 1=标准, 2=详细
auto_fix = false         # 是否自动修复（不建议开启）
confirm_before_fix = true # 修复前是否需要确认
offline = false          # 完全禁止网络请求

[llm]
strategy = "local"       # AI 策略: local / cloud / hybrid

[llm.local]
endpoint = "http://localhost:11434"  # Ollama 服务地址
model = "qwen2.5:3b"                # 本地模型名称

[llm.cloud]
provider = "qwen"        # 云端供应商: qwen / deepseek / moonshot / custom
model = "qwen-plus"      # 云端模型名称
api_key_env = "QWEN_API_KEY"  # API Key 环境变量名
endpoint = "https://dashscope.aliyuncs.com/compatible-mode/v1"  # API 端点

[web]
host = "127.0.0.1"       # Web 仪表盘监听地址
port = 8080              # Web 仪表盘监听端口

[daemon]
interval = 3600          # 守护进程巡检间隔（秒）
notify = true            # 是否发送桌面通知
```

### 数据目录结构

```
~/.kylin-doctor/
├── config.toml           # 配置文件
├── suid_whitelist.txt    # SUID 文件白名单（可选，每行一个路径）
└── knowledge/            # 知识库目录（自动创建）
    ├── index.json        # 文档索引
    └── raw_docs/         # 原始文档
```

### 环境变量

| 变量名 | 用途 | 默认值 |
|--------|------|--------|
| `HOST` | Web 仪表盘监听地址（覆盖配置文件） | `127.0.0.1` |
| `PORT` | Web 仪表盘监听端口（覆盖配置文件） | `8080` |
| `QWEN_API_KEY` | 通义千问 API Key（云端模式必需） | 无 |
| `DEEPSEEK_API_KEY` | DeepSeek API Key（使用 deepseek 时） | 无 |
| `MOONSHOT_API_KEY` | Moonshot API Key（使用 moonshot 时） | 无 |

---

## CLI 部署

### 快速验证

```bash
# 全面扫描
kylin-doctor scan

# 快速扫描（跳过耗时模块）
kylin-doctor scan --quick

# 只扫描安全模块
kylin-doctor scan --module security
```

### 典型使用流程

```bash
# 第一步：全面扫描
kylin-doctor scan

# 第二步：查看修复建议
kylin-doctor fix --dry-run

# 第三步：修复严重问题
kylin-doctor fix --critical-only

# 第四步：生成报告存档
kylin-doctor report --format html --output report.html
```

### 输出说明

CLI 输出使用 emoji 图标标识严重程度：

| 图标 | 含义 |
|------|------|
| 🔴 | 严重问题（Critical）— 需要立即处理 |
| ⚠️ | 警告（Warning）— 建议尽快处理 |
| ℹ️ | 信息（Info）— 可选优化项 |
| ✅ | 正常/成功 |
| ❌ | 失败 |

### 退出码

| 退出码 | 含义 |
|--------|------|
| `0` | 正常，无警告无严重问题 |
| `1` | 存在警告 |
| `2` | 存在严重问题 |

---

## Web 仪表盘部署

### 方式一：通过 CLI 内置命令

```bash
# 默认启动（127.0.0.1:8080）
kylin-doctor serve

# 自定义端口
kylin-doctor serve --port 9090

# 监听所有网络接口（局域网访问）
kylin-doctor serve --host 0.0.0.0 --port 8080
```

### 方式二：独立 Web 二进制

```bash
# 使用环境变量配置
HOST=0.0.0.0 PORT=9090 kylin-doctor-web

# 或使用配置文件（~/.kylin-doctor/config.toml 中的 [web] 段）
kylin-doctor-web
```

### 方式三：systemd 服务

创建 `/etc/systemd/system/kylin-doctor-web.service`：

```ini
[Unit]
Description=kylin-doctor Web Dashboard
After=network.target

[Service]
Type=simple
User=root
ExecStart=/usr/local/bin/kylin-doctor-web
Environment=HOST=0.0.0.0
Environment=PORT=8080
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

启用并启动：

```bash
sudo systemctl daemon-reload
sudo systemctl enable kylin-doctor-web
sudo systemctl start kylin-doctor-web

# 查看状态
sudo systemctl status kylin-doctor-web

# 查看日志
sudo journalctl -u kylin-doctor-web -f
```

### 访问仪表盘

打开浏览器访问：`http://<服务器IP>:8080`

仪表盘功能：

| 功能 | 说明 |
|------|------|
| 系统概览 | 主机名、CPU、内存、内核、运行时间 |
| 健康雷达图 | 五大模块健康评分可视化 |
| 问题分布图 | 严重/警告/信息数量饼图 |
| 全量扫描 | 一键扫描所有模块 |
| 实时扫描 | WebSocket 推送扫描进度 |
| 模块扫描 | 单独扫描某个模块 |
| 报告导出 | 导出 JSON / HTML 格式报告 |
| AI 对话 | 浮动面板，与 AI 助手实时对话 |

### 反向代理配置（Nginx）

```nginx
server {
    listen 80;
    server_name kylin-doctor.local;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;

        # WebSocket 支持
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_read_timeout 86400;
    }
}
```

---

## AI 模型配置

### 本地模型（Ollama）

#### 安装 Ollama

```bash
# 一键安装
curl -fsSL https://ollama.com/install.sh | sh

# 启动服务
ollama serve &

# 拉取推荐模型（7B 参数，约 4.7GB）
ollama pull qwen2.5:3b

# 拉取嵌入模型（用于知识库 RAG，约 274MB）
ollama pull nomic-embed-text

# 验证
ollama list
curl http://localhost:11434/api/tags
```

#### 推荐模型配置

| 用途 | 模型 | 大小 | 运行内存 | 说明 |
|------|------|------|----------|------|
| 对话诊断 | `qwen2.5:3b` | ~2 GB | ~3 GB | **默认推荐**，适配 8GB 内存终端 |
| 对话诊断 | `qwen2.5:1.5b` | ~1 GB | ~1.5 GB | 更轻量，内存极度紧张时选用 |
| 对话诊断 | `qwen2.5:7b` | ~4.7 GB | ~5 GB | 高质量，需要 16GB+ 内存 |
| 文本嵌入 | `nomic-embed-text` | ~274 MB | ~300 MB | 知识库向量化专用 |

#### 配置文件

```toml
[llm]
strategy = "local"

[llm.local]
endpoint = "http://localhost:11434"
model = "qwen2.5:3b"
```

#### 使用方式

```bash
# 默认使用本地模型
kylin-doctor chat

# 单次提问
kylin-doctor chat "我的系统为什么变慢了？"

# 交互模式
kylin-doctor chat
> 扫描一下我的系统
> /scan
> 退出
```

### 云端模型

#### 通义千问（Qwen）

```bash
# 设置 API Key
export QWEN_API_KEY="sk-your-api-key"
```

```toml
[llm]
strategy = "cloud"

[llm.cloud]
provider = "qwen"
model = "qwen-plus"
api_key_env = "QWEN_API_KEY"
endpoint = "https://dashscope.aliyuncs.com/compatible-mode/v1"
```

#### DeepSeek

```bash
export DEEPSEEK_API_KEY="sk-your-api-key"
```

```toml
[llm]
strategy = "cloud"

[llm.cloud]
provider = "deepseek"
model = "deepseek-chat"
api_key_env = "DEEPSEEK_API_KEY"
endpoint = "https://api.deepseek.com/v1"
```

#### Moonshot（月之暗面）

```bash
export MOONSHOT_API_KEY="sk-your-api-key"
```

```toml
[llm]
strategy = "cloud"

[llm.cloud]
provider = "moonshot"
model = "moonshot-v1-8k"
api_key_env = "MOONSHOT_API_KEY"
endpoint = "https://api.moonshot.cn/v1"
```

#### 自定义 OpenAI 兼容接口

```toml
[llm]
strategy = "cloud"

[llm.cloud]
provider = "custom"
model = "your-model-name"
api_key_env = "YOUR_API_KEY_ENV"
endpoint = "https://your-api-endpoint/v1"
```

### 混合模式

混合模式优先使用本地模型，本地不可用时自动回退到云端：

```toml
[llm]
strategy = "hybrid"
```

```bash
# 通过命令行参数切换
kylin-doctor --provider local chat     # 强制本地
kylin-doctor --provider cloud chat     # 强制云端
kylin-doctor --provider hybrid chat    # 混合模式
```

### AI Function Calling

kylin-doctor 支持 Function Calling，AI 可以自动调用诊断工具：

| 工具名 | 功能 |
|--------|------|
| `scan_system` | 扫描系统健康状态 |
| `scan_hardware` | 扫描硬件状态 |
| `scan_software` | 扫描软件生态 |
| `scan_security` | 扫描安全配置 |
| `scan_performance` | 扫描性能指标 |
| `scan_all` | 全量扫描 |

当用户提问涉及系统状态时，AI 会自动调用相关工具获取实时数据，然后基于数据给出诊断建议。

---

## 知识库配置

知识库（RAG）允许 kylin-doctor 基于自定义文档回答问题。

### 添加文档

```bash
# 添加单个文件
kylin-doctor knowledge add /path/to/document.md

# 递归添加目录
kylin-doctor knowledge add /path/to/docs/ --recursive

# 支持的文件格式
# .txt, .md, .rst, .conf, .cfg, .log
```

### 管理知识库

```bash
# 查看已添加的文档
kylin-doctor knowledge list

# 查看统计信息
kylin-doctor knowledge status

# 删除文档
kylin-doctor knowledge remove doc_0
```

### 生成向量嵌入

```bash
# 需要 Ollama 运行中，且已拉取 nomic-embed-text 模型
ollama pull nomic-embed-text
kylin-doctor knowledge embed
```

### 测试检索

```bash
# 语义搜索（需要嵌入）
kylin-doctor knowledge test "如何配置打印机"

# 关键词搜索（无需嵌入，自动回退）
kylin-doctor knowledge test "打印机配置"
```

### 与 AI 对话集成

知识库内容会自动注入到 AI 对话的上下文中。添加文档并生成嵌入后，AI 可以基于知识库内容回答问题：

```bash
# 添加麒麟系统文档
kylin-doctor knowledge add /usr/share/doc/kylin-* --recursive
kylin-doctor knowledge embed

# AI 将基于文档内容回答
kylin-doctor chat "如何配置麒麟系统的网络？"
```

---

## 系统服务部署

### 定时巡检（cron）

```bash
# 编辑 crontab
crontab -e

# 每天凌晨 2 点执行扫描并保存报告
0 2 * * * /usr/local/bin/kylin-doctor report --format html --output /var/log/kylin-doctor/daily-$(date +\%Y\%m\%d).html 2>/dev/null

# 每小时执行一次快速扫描
0 * * * * /usr/local/bin/kylin-doctor scan --quick >> /var/log/kylin-doctor/scan.log 2>&1
```

### 定时报告脚本

创建 `/usr/local/bin/kylin-doctor-report.sh`：

```bash
#!/bin/bash
set -euo pipefail

REPORT_DIR="/var/log/kylin-doctor"
DATE=$(date +%Y%m%d_%H%M%S)
mkdir -p "$REPORT_DIR"

# 生成 HTML 报告
kylin-doctor report --format html --output "$REPORT_DIR/report-$DATE.html"

# 生成 JSON 报告
kylin-doctor report --format json --output "$REPORT_DIR/report-$DATE.json"

# 清理 30 天前的报告
find "$REPORT_DIR" -name "report-*" -mtime +30 -delete

echo "[$(date)] 报告已生成: $REPORT_DIR/report-$DATE.html"
```

```bash
sudo chmod +x /usr/local/bin/kylin-doctor-report.sh
```

---

## Docker 部署

### Dockerfile

```dockerfile
FROM rust:1.77-bookworm AS builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    procps coreutils systemd smartmontools pciutils usbutils \
    dmidecode lm-sensors dpkg apt fontconfig iproute2 iputils-ping \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/kylin-doctor /usr/local/bin/
COPY --from=builder /app/target/release/kylin-doctor-web /usr/local/bin/

EXPOSE 8080

CMD ["kylin-doctor-web"]
```

### 构建与运行

```bash
# 构建镜像
docker build -t kylin-doctor .

# 运行容器
docker run -d \
  --name kylin-doctor \
  --privileged \
  -p 8080:8080 \
  -v /proc:/host/proc:ro \
  -v /sys:/host/sys:ro \
  -v /etc:/host/etc:ro \
  kylin-doctor
```

> ⚠️ Docker 部分功能受限，因为容器内无法完整访问宿主机的 `/proc`、`/sys` 等文件系统。

---

## 故障排查

### 编译失败

```bash
# 确保 Rust 版本足够新
rustc --version  # 需要 1.70+

# 更新 Rust
rustup update

# 清理并重新构建
cargo clean
cargo build --release
```

### Ollama 连接失败

```bash
# 检查 Ollama 是否运行
curl http://localhost:11434/api/tags

# 如果失败，启动 Ollama
ollama serve &

# 检查端口是否被占用
ss -tlnp | grep 11434
```

### 权限不足

某些检测项需要 root 权限：

```bash
# 需要 root 的检测项：
# - 读取 /etc/shadow（安全模块）
# - 运行 smartctl（硬件模块）
# - 运行 ss -tlnp（安全/性能模块）
# - 读取 dmesg（系统模块）

# 使用 sudo 运行
sudo kylin-doctor scan
```

### Web 仪表盘无法访问

```bash
# 检查服务是否运行
ss -tlnp | grep 8080

# 检查防火墙
sudo ufw status
sudo ufw allow 8080/tcp

# 检查 SELinux（如果有）
sudo setsebool -P httpd_can_network_connect 1
```

### 检测结果为空

```bash
# 使用详细模式查看跳过原因
kylin-doctor scan -v 2

# 检查依赖工具是否安装
which systemctl smartctl lspci lsblk ss dmesg
```

### 知识库检索无结果

```bash
# 检查知识库状态
kylin-doctor knowledge status

# 确认已生成嵌入
kylin-doctor knowledge embed

# 尝试关键词搜索（无需嵌入）
kylin-doctor knowledge test "你的问题"
```

---

## 性能调优

### 快速扫描模式

```bash
# 跳过硬件检测（磁盘速度采样）和性能检测（CPU/IO 采样）
kylin-doctor scan --quick
```

### 并发扫描

Web 仪表盘的 REST API 支持并发请求，适合监控系统定期调用：

```bash
# curl 调用 API
curl http://localhost:8080/api/scan/system
curl http://localhost:8080/api/status
```

### 资源消耗

| 操作 | CPU | 内存 | 耗时 |
|------|-----|------|------|
| 快速扫描 | 低 | ~20 MB | <5 秒 |
| 全量扫描 | 中 | ~30 MB | 10-30 秒 |
| AI 对话 | 低（本地）/ 无（云端） | ~50 MB | 取决于模型 |
| 知识库嵌入 | 中 | ~100 MB | 取决于文档量 |
