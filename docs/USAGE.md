# kylin-doctor 使用说明

> 银河麒麟桌面系统自我诊断工具 — 完整使用指南

---

## 目录

- [快速上手](#快速上手)
- [CLI 命令详解](#cli-命令详解)
  - [scan — 系统扫描](#scan--系统扫描)
  - [fix — 问题修复](#fix--问题修复)
  - [report — 生成报告](#report--生成报告)
  - [chat — AI 对话](#chat--ai-对话)
  - [knowledge — 知识库管理](#knowledge--知识库管理)
  - [serve — 启动 Web 仪表盘](#serve--启动-web-仪表盘)
- [Web 仪表盘使用](#web-仪表盘使用)
- [诊断模块详解](#诊断模块详解)
- [AI 助手使用](#ai-助手使用)
- [常见场景](#常见场景)
- [输出格式参考](#输出格式参考)
- [最佳实践](#最佳实践)

---

## 快速上手

### 一分钟体验

```bash
# 1. 全面扫描你的系统
kylin-doctor scan

# 2. 查看可以自动修复的问题
kylin-doctor fix --dry-run

# 3. 启动 Web 仪表盘
kylin-doctor serve
# 浏览器打开 http://127.0.0.1:8080
```

### 典型工作流

```
扫描 → 分析 → 修复 → 验证 → 报告
 │       │       │       │       │
scan    chat    fix    scan    report
```

---

## CLI 命令详解

### 全局选项

所有子命令共享以下全局选项：

```bash
kylin-doctor [全局选项] <子命令> [子命令选项]

# 全局选项
-v, --verbose <级别>      # 输出详细程度: 0=简要, 1=标准, 2=详细 (默认: 1)
-p, --provider <策略>     # AI 模型策略: local / cloud / hybrid (默认: local)
```

**示例：**

```bash
# 详细模式扫描
kylin-doctor -v 2 scan

# 使用云端 AI 对话
kylin-doctor -p cloud chat
```

---

### scan — 系统扫描

全面检测系统状态，涵盖硬件、系统、软件、安全、性能五大维度。

```bash
kylin-doctor scan [选项]
```

| 选项 | 缩写 | 说明 |
|------|------|------|
| `--module <模块>` | `-m` | 只扫描指定模块 |
| `--quick` | `-q` | 快速扫描，跳过耗时检测 |

**模块列表：**

| 模块名 | 说明 | 耗时 |
|--------|------|------|
| `system` | 系统健康（磁盘、服务、进程、内核日志、负载） | 快 |
| `hardware` | 硬件状态（温度、内存、磁盘健康、GPU、网卡） | 慢 ⏱️ |
| `software` | 软件生态（包管理、字体、运行时、兼容层） | 快 |
| `security` | 安全审计（密码、权限、SSH、防火墙、漏洞） | 快 |
| `performance` | 性能分析（CPU、内存、磁盘IO、网络、桌面合成器） | 慢 ⏱️ |

**使用示例：**

```bash
# 全面扫描
kylin-doctor scan

# 快速扫描（跳过 hardware 和 performance）
kylin-doctor scan --quick

# 只扫描安全模块
kylin-doctor scan --module security

# 只扫描系统模块
kylin-doctor scan -m system

# 详细输出
kylin-doctor -v 2 scan
```

**输出示例：**

```
🔍 kylin-doctor 系统诊断

├── [████████████████████] 5/5 正在扫描 performance...
└── 扫描完成

📋 system [⚠️  2 个警告]
   扫描耗时: 12ms
   ⚠️  [system-disk-warning-_home] /home 磁盘空间偏高 (85%)
      💡 建议: 清理临时文件

   ⚠️  [system-zombie-processes] 发现 2 个僵尸进程

📋 security [🔴 1 个严重] [⚠️  1 个警告]
   扫描耗时: 45ms
   🔴 [security-ssh-root-login] SSH 允许 root 登录
      💡 建议: 修改 /etc/ssh/sshd_config 中 PermitRootLogin 为 no

   ⚠️  [security-firewall-inactive] 防火墙未启用
      💡 建议: sudo ufw enable

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🔴 严重: 1  ⚠️  警告: 3  ℹ️  信息: 0
存在严重问题，请优先处理严重项。
```

---

### fix — 问题修复

扫描并自动修复检测到的问题。

```bash
kylin-doctor fix [选项]
```

| 选项 | 缩写 | 说明 |
|------|------|------|
| `--module <模块>` | `-m` | 只修复指定模块的问题 |
| `--dry-run` | | 预览修复操作，不实际执行 |
| `--yes` | `-y` | 跳过确认直接修复（危险） |
| `--auto-only` | | 只修复可自动修复的问题 |
| `--critical-only` | | 只修复严重问题 |

**使用示例：**

```bash
# 预览所有可修复的问题（不执行）
kylin-doctor fix --dry-run

# 只修复严重问题
kylin-doctor fix --critical-only

# 只修复可自动修复的问题
kylin-doctor fix --auto-only

# 只修复安全模块的问题
kylin-doctor fix --module security

# 跳过确认直接修复（谨慎使用）
kylin-doctor fix --yes

# 组合使用：只自动修复安全模块的严重问题
kylin-doctor fix -m security --critical-only --auto-only
```

**风险等级说明：**

| 风险等级 | 颜色 | 说明 |
|----------|------|------|
| low | 🟢 绿色 | 低风险，安全执行 |
| medium | 🟡 黄色 | 中等风险，建议确认 |
| high | 🔴 红色 | 高风险，务必谨慎 |

**最佳实践：**

```bash
# 始终先预览
kylin-doctor fix --dry-run

# 确认无误后执行
kylin-doctor fix

# 修复后重新扫描验证
kylin-doctor scan
```

---

### report — 生成诊断报告

生成可分享的诊断报告文件。

```bash
kylin-doctor report [选项]
```

| 选项 | 缩写 | 说明 |
|------|------|------|
| `--format <格式>` | `-f` | 输出格式: `json` 或 `html`（默认: json） |
| `--output <路径>` | `-o` | 保存到文件（默认: 输出到 stdout） |

**使用示例：**

```bash
# JSON 格式输出到终端
kylin-doctor report

# JSON 格式保存到文件
kylin-doctor report --format json --output report.json

# HTML 格式保存到文件（推荐用于分享）
kylin-doctor report --format html --output report.html

# 简写形式
kylin-doctor report -f html -o report.html
```

**JSON 报告结构：**

```json
{
  "version": "0.1.0",
  "status": "warning",
  "summary": {
    "info": 3,
    "warning": 5,
    "critical": 1
  },
  "modules": [
    {
      "module": "system",
      "duration_ms": 12,
      "summary": { "info": 0, "warning": 2, "critical": 0 },
      "findings": [...]
    }
  ]
}
```

**HTML 报告特性：**

- 自包含单文件，可直接在浏览器打开
- 状态徽章（正常/警告/严重）
- 按模块分组的检测结果
- 严重程度颜色标识
- 修复建议与命令
- 适合打印和邮件分享

---

### chat — AI 对话

接入 AI 大模型，智能诊断系统问题。

```bash
kylin-doctor chat [问题]
```

**使用方式：**

```bash
# 单次提问
kylin-doctor chat "我的系统为什么变慢了？"

# 交互模式（REPL）
kylin-doctor chat

# 使用云端模型
kylin-doctor -p cloud chat

# 混合模式（本地优先，失败回退云端）
kylin-doctor -p hybrid chat
```

**交互模式命令：**

| 命令 | 说明 |
|------|------|
| `scan` / `扫描` | 执行全量扫描 |
| `help` / `帮助` | 显示帮助信息 |
| `exit` / `quit` / `退出` | 退出对话 |

**对话示例：**

```
🤖 kylin-doctor AI 诊断助手
   模型: qwen2.5:7b (local)
   输入 /help 查看帮助，输入 exit 退出

you> 我的系统最近变得很慢，可能是什么原因？

🤖 让我先扫描一下您的系统状态。

[正在调用工具: scan_all]

🤖 根据扫描结果，发现以下可能导致系统变慢的原因：

1. **内存使用率 92%**（严重） — 物理内存几乎耗尽，系统大量使用 Swap
   - 建议: 关闭不必要的程序，或考虑升级内存

2. **CPU 使用率 85%**（警告） — CPU 负载较高
   - 建议: 使用 `top` 查看占用 CPU 最高的进程

3. **磁盘 I/O 延迟 45ms**（警告） — 磁盘读写较慢
   - 建议: 检查磁盘健康状态，考虑升级到 SSD

您可以运行 `kylin-doctor fix --dry-run` 查看可修复的问题。
```

**Function Calling：**

AI 助手具备 Function Calling 能力，当用户提问涉及系统状态时，会自动调用诊断工具：

- 问"系统为什么慢" → 自动调用 `scan_performance`
- 问"安全吗" → 自动调用 `scan_security`
- 问"硬件怎么样" → 自动调用 `scan_hardware`
- 说"全面检查" → 自动调用 `scan_all`

---

### knowledge — 知识库管理

管理本地知识库，为 AI 对话提供上下文。

```bash
kylin-doctor knowledge <子命令> [参数]
```

| 子命令 | 说明 |
|--------|------|
| `add <路径>` | 添加文档到知识库 |
| `list` | 列出所有文档 |
| `status` | 查看统计信息 |
| `remove <ID>` | 删除文档 |
| `embed` | 生成向量嵌入 |
| `test <查询>` | 测试检索效果 |

**使用示例：**

```bash
# 添加单个文件
kylin-doctor knowledge add /usr/share/doc/systemd/README

# 递归添加目录
kylin-doctor knowledge add /usr/share/doc/kylin-manual --recursive

# 查看已添加的文档
kylin-doctor knowledge list

# 查看统计
kylin-doctor knowledge status
# 输出:
# 📊 知识库统计
#    文档数: 15
#    分块数: 234
#    已嵌入: 234 (100.0%)

# 生成向量嵌入（需要 Ollama + nomic-embed-text）
kylin-doctor knowledge embed

# 测试检索
kylin-doctor knowledge test "如何配置网络"

# 删除文档
kylin-doctor knowledge remove doc_0
```

**支持的文件格式：**

| 格式 | 说明 |
|------|------|
| `.txt` | 纯文本 |
| `.md` | Markdown |
| `.rst` | reStructuredText |
| `.conf` | 配置文件 |
| `.cfg` | 配置文件 |
| `.log` | 日志文件 |

**知识库工作原理：**

```
文档 → 分块(500字/块,50字重叠) → 向量嵌入 → 本地存储
                                                ↓
用户提问 → 向量相似度搜索 → Top-K 相关分块 → 注入 AI 上下文
```

---

### serve — 启动 Web 仪表盘

启动基于浏览器的可视化仪表盘。

```bash
kylin-doctor serve [选项]
```

| 选项 | 缩写 | 说明 |
|------|------|------|
| `--port <端口>` | `-p` | 监听端口（默认: 8080） |
| `--host <地址>` | | 监听地址（默认: 127.0.0.1） |

**使用示例：**

```bash
# 默认启动
kylin-doctor serve

# 自定义端口
kylin-doctor serve --port 9090

# 局域网可访问
kylin-doctor serve --host 0.0.0.0 --port 8080
```

---

## Web 仪表盘使用

### 界面布局

```
┌─────────────────────────────────────────────────────┐
│  🔍 kylin-doctor 系统诊断仪表盘                      │
├──────────┬──────────┬──────────┬──────────┬──────────┤
│ 🖥️ 主机名 │ 💻 CPU   │ 🧠 内存   │ ⏱️ 运行时间 │ 🐧 内核  │
│ kyunix   │ 23%      │ 67%      │ 12.5h    │ 5.15.0  │
├──────────┴──────────┴──────────┴──────────┴──────────┤
│                                                      │
│  [全量扫描] [实时扫描] [system] [hardware] [software]  │
│  [security] [performance]                            │
│                                                      │
│  ┌──────────────┐  ┌──────────────┐                  │
│  │  健康雷达图   │  │  问题分布图   │                  │
│  │  (ECharts)   │  │  (ECharts)   │                  │
│  └──────────────┘  └──────────────┘                  │
│                                                      │
│  📋 检测结果                                          │
│  ┌─────────────────────────────────────────────────┐ │
│  │ 🔴 [security] SSH 允许 root 登录                 │ │
│  │    💡 修改 sshd_config: PermitRootLogin no       │ │
│  ├─────────────────────────────────────────────────┤ │
│  │ ⚠️  [system] /home 磁盘空间 85%                  │ │
│  │    💡 清理临时文件: rm -rf ~/.cache/thumbnails/*  │ │
│  └─────────────────────────────────────────────────┘ │
│                                                      │
│  [📄 JSON 报告] [📄 HTML 报告]                        │
└─────────────────────────────────────────────────────┘
                                    ┌─────────────────┐
                                    │ 🤖 AI 助手       │
                                    │ ┌─────────────┐ │
                                    │ │ 对话内容...   │ │
                                    │ │             │ │
                                    │ ├─────────────┤ │
                                    │ │ [输入消息]   │ │
                                    │ └─────────────┘ │
                                    └─────────────────┘
```

### 操作说明

| 操作 | 说明 |
|------|------|
| **全量扫描** | 点击按钮，依次扫描所有 5 个模块 |
| **实时扫描** | 通过 WebSocket 实时推送扫描进度 |
| **模块扫描** | 点击单个模块按钮，只扫描该模块 |
| **导出报告** | 点击 JSON/HTML 按钮，在新标签页打开报告 |
| **AI 对话** | 点击右下角 💬 按钮，打开/关闭 AI 对话面板 |

### AI 对话面板

- 浮动面板，可拖动
- 支持实时对话
- 输入 `/scan` 触发扫描
- 显示工具调用过程
- 支持 Markdown 渲染

---

## 诊断模块详解

### 系统模块 (system)

| 检测项 | 检测方式 | 严重阈值 | 警告阈值 |
|--------|----------|----------|----------|
| 磁盘空间 | `df -h` | ≥90% | ≥80% |
| 失败服务 | `systemctl list-units --state=failed` | - | 任何失败 |
| 僵尸进程 | `ps -eo pid,ppid,stat,comm` | - | >5 个 |
| 内核错误 | `dmesg --level=err,crit,alert,emerg` | - | 任何错误 |
| 系统负载 | `/proc/loadavg` / `nproc` | 负载/核数 >2.0 | >1.0 |

### 硬件模块 (hardware)

| 检测项 | 检测方式 | 严重阈值 | 警告阈值 |
|--------|----------|----------|----------|
| CPU 温度 | `/sys/class/thermal/thermal_zone*/temp` | >95°C | >80°C |
| GPU 状态 | `lspci` + `nvidia-smi` | GPU >90°C | GPU >80°C, VRAM >90% |
| 内存使用 | `/proc/meminfo` | >95% | >85% |
| 磁盘健康 | `smartctl -H` | SMART 失败 | - |
| 磁盘寿命 | `smartctl -A` | SSD ≤10% | SSD ≤30% |
| 磁盘IO错误 | `/proc/diskstats` | - | >0 错误 |
| 网卡状态 | `/sys/class/net/*/operstate` | - | 错误 >100 |
| USB 设备 | `lsusb` | - | 打印机无 CUPS |
| 主板信息 | `/sys/class/dmi/id/` | - | 电池 <2700mV |

### 软件模块 (software)

| 检测项 | 检测方式 | 说明 |
|--------|----------|------|
| 包管理状态 | `dpkg --audit` | 检查损坏的包 |
| 可用更新 | `apt list --upgradable` | >50 个更新时警告 |
| 软件源 | `/etc/apt/sources.list` | 空源/密钥错误 |
| 运行时环境 | `which python3 java node` | 检查常用运行时 |
| 中文字体 | `fc-list :lang=zh` | 无中文字体时警告 |
| 字体渲染 | `fc-match --verbose` | CJK 优先级/提示/抗锯齿 |
| Wine 兼容 | `which wine` | Wine 安装状态 |
| Android 兼容 | Anbox/Waydroid | 安装与运行状态 |
| 依赖冲突 | `apt-get check` | 依赖关系问题 |
| 通用包 | Snap/Flatpak | 功能状态 |

### 安全模块 (security)

| 检测项 | 检测方式 | 严重阈值 | 警告阈值 |
|--------|----------|----------|----------|
| 空密码 | `/etc/shadow` | 任何空密码 | - |
| UID=0 账户 | `/etc/passwd` | 非 root 的 UID 0 | - |
| 过期账户 | `/etc/shadow` | - | 过期账户 |
| SUID 文件 | `find /usr/bin ...` | - | 未知 SUID |
| 目录权限 | `stat` | - | 关键目录权限错误 |
| SSH 配置 | `sshd_config` | root 登录/空密码 | 密码认证开启 |
| 防火墙 | `ufw status` | - | 防火墙未启用 |
| 开放端口 | `ss -tlnp` | - | 高危端口暴露 |
| 登录失败 | `auth.log` | >100 次 | >20 次 |
| 审计日志 | `auditd` 状态 | - | sudo 失败 >10 次 |
| 已知漏洞 | 内核版本/安全更新 | - | 待更新/ASLR 异常 |
| 密码策略 | `/etc/login.defs` | - | 策略过于宽松 |

### 性能模块 (performance)

| 检测项 | 检测方式 | 严重阈值 | 警告阈值 |
|--------|----------|----------|----------|
| CPU 使用率 | `/proc/stat` 采样 | >95% | >80% |
| CPU 调度 | `/proc/schedstat` | - | 延迟 >10ms |
| 负载趋势 | `/proc/loadavg` | - | 上升趋势且 >1.5 |
| 内存性能 | `/proc/meminfo` | - | Swap >80% |
| 内存碎片 | `/proc/buddyinfo` | - | 高阶块 <5% |
| 磁盘 I/O | `/proc/diskstats` 采样 | 延迟 >100ms | 延迟 >20ms |
| 磁盘 IOPS | `/proc/diskstats` | - | 队列深度 >32 |
| 网络连接 | `ss -s` | - | TIME_WAIT >5000 |
| 网络延迟 | `ping` 网关 | - | RTT >50ms |
| 网络带宽 | `/sys/class/net/*/speed` | - | 错误率 >0.1% |
| 桌面合成器 | 进程检测 + `xrandr` | - | CPU >30% |
| 电源状态 | `/sys/class/power_supply/` | 电池耗尽 | 电量 <10% |
| IO 调度器 | `/sys/block/*/queue/scheduler` | - | SSD 用 cfq |

---

## AI 助手使用

### 提问技巧

```bash
# ✅ 好的提问 — 具体、明确
"我的 Firefox 经常崩溃，怎么排查？"
"如何配置 Kylin 的打印机？"
"系统日志里有很多 OOM 错误，怎么解决？"

# ❌ 不好的提问 — 太模糊
"系统有问题"
"帮帮我"
```

### 常见问题示例

```bash
# 性能问题
kylin-doctor chat "系统运行缓慢，如何排查？"
kylin-doctor chat "内存使用率很高怎么办？"
kylin-doctor chat "磁盘 IO 延迟高是什么原因？"

# 安全问题
kylin-doctor chat "如何加固 SSH 安全？"
kylin-doctor chat "怎么检查系统有没有被入侵？"
kylin-doctor chat "防火墙怎么配置？"

# 软件问题
kylin-doctor chat "apt 更新失败怎么解决？"
kylin-doctor chat "中文字体显示乱码怎么办？"
kylin-doctor chat "如何安装 Wine 运行 Windows 程序？"

# 硬件问题
kylin-doctor chat "CPU 温度太高怎么处理？"
kylin-doctor chat "如何检查磁盘是否快要坏了？"
kylin-doctor chat "怎么看显卡驱动是否正常？"

# 系统管理
kylin-doctor chat "如何查看系统日志？"
kylin-doctor chat "怎么设置定时任务？"
kylin-doctor chat "如何清理系统垃圾文件？"
```

---

## 常见场景

### 场景一：日常巡检

```bash
# 快速检查系统状态
kylin-doctor scan --quick

# 如果有警告，查看修复建议
kylin-doctor fix --dry-run

# 修复低风险问题
kylin-doctor fix --auto-only
```

### 场景二：安全审计

```bash
# 全面安全扫描
kylin-doctor scan --module security

# 生成安全报告
kylin-doctor report --format html --output security-audit.html

# 修复安全问题
kylin-doctor fix --module security --critical-only
```

### 场景三：性能调优

```bash
# 性能分析
kylin-doctor scan --module performance

# AI 辅助分析
kylin-doctor chat "根据扫描结果，如何优化系统性能？"

# 硬件瓶颈排查
kylin-doctor scan --module hardware
```

### 场景四：新机验收

```bash
# 全面检测新机器
kylin-doctor scan

# 生成验收报告
kylin-doctor report --format html --output acceptance-report.html

# AI 总结
kylin-doctor chat "总结一下这台机器的检测结果，有什么需要注意的？"
```

### 场景五：故障排查

```bash
# 先扫描获取系统状态
kylin-doctor scan

# 向 AI 描述问题
kylin-doctor chat "系统经常卡顿，扫描结果显示内存使用 95%、Swap 使用 60%，怎么解决？"

# 逐步修复
kylin-doctor fix --dry-run
kylin-doctor fix --module system
```

### 场景六：批量部署

```bash
# 在目标机器上执行扫描并保存报告
for host in server1 server2 server3; do
    ssh $host "kylin-doctor report -f json -o /tmp/report.json"
    scp $host:/tmp/report.json ./reports/$host.json
done
```

---

## 输出格式参考

### 终端输出

```
🔍 kylin-doctor 系统诊断

├── [████████████████████] 5/5 正在扫描 performance...
└── 扫描完成

📋 system [✅ 正常]
   扫描耗时: 12ms

📋 hardware [⚠️  1 个警告]
   扫描耗时: 3200ms
   ⚠️  [hardware-cpu-temp-high] CPU 温度偏高 (82°C)
      证据: thermal_zone0: type=x86_pkg_temp temp=82000
      💡 建议: 检查散热器是否积灰，考虑更换硅脂

📋 software [ℹ️  2 个信息]
   扫描耗时: 85ms
   ℹ️  [software-updates-available] 有 23 个软件包可更新
   ℹ️  [software-python-missing] Python3 未安装

📋 security [🔴 1 个严重] [⚠️  1 个警告]
   扫描耗时: 45ms
   🔴 [security-ssh-root-login] SSH 允许 root 登录
      证据: PermitRootLogin yes
      💡 修复: sudo sed -i 's/PermitRootLogin yes/PermitRootLogin no/' /etc/ssh/sshd_config
      ⚠️  风险: 低

   ⚠️  [security-firewall-inactive] 防火墙未启用
      证据: Status: inactive
      💡 修复: sudo ufw enable
      ⚠️  风险: 低

📋 performance [⚠️  1 个警告]
   扫描耗时: 2100ms
   ⚠️  [perf-swap-high] Swap 使用率过高 (45%)
      证据: SwapTotal=2097152kB SwapFree=1153434kB usage=45.0%

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🔴 严重: 1  ⚠️  警告: 3  ℹ️  信息: 2
存在严重问题，请优先处理严重项。
```

### Finding 结构

每个检测发现包含以下字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | String | 唯一标识，如 `security-ssh-root-login` |
| `module` | String | 所属模块 |
| `severity` | Enum | `info` / `warning` / `critical` |
| `title` | String | 简短标题 |
| `description` | String | 详细描述 |
| `evidence` | String | 检测证据 |
| `fix` | Object? | 修复方案（可选） |
| `auto_fixable` | bool | 是否可自动修复 |

---

## 最佳实践

### 1. 定期扫描

```bash
# 建议每周至少一次全量扫描
kylin-doctor scan

# 每天快速检查
kylin-doctor scan --quick
```

### 2. 先预览后修复

```bash
# 始终先用 --dry-run 预览
kylin-doctor fix --dry-run

# 确认安全后再执行
kylin-doctor fix
```

### 3. 保留报告

```bash
# 重要操作前保存报告
kylin-doctor report -f html -o "before-upgrade-$(date +%Y%m%d).html"
```

### 4. 使用知识库

```bash
# 添加系统文档到知识库
kylin-doctor knowledge add /usr/share/doc/ --recursive
kylin-doctor knowledge embed

# AI 将基于文档给出更准确的建议
kylin-doctor chat "麒麟系统如何配置 VPN？"
```

### 5. 选择合适的 AI 模型

| 场景 | 推荐策略 |
|------|----------|
| 有网络，追求质量 | `--provider cloud` |
| 离线环境 | `--provider local` |
| 不确定网络状况 | `--provider hybrid` |

### 6. 关注严重问题

```bash
# 优先处理严重问题
kylin-doctor fix --critical-only

# 再处理警告
kylin-doctor fix --auto-only
```

### 7. 自定义 SUID 白名单

如果某些 SUID 文件是已知安全的，添加到白名单避免重复告警：

```bash
# 编辑白名单文件
cat >> ~/.kylin-doctor/suid_whitelist.txt << 'EOF'
# 自定义白名单
/usr/bin/my-custom-tool
/usr/sbin/my-admin-tool
EOF
```

### 8. 离线环境使用

```bash
# 配置离线模式
cat >> ~/.kylin-doctor/config.toml << 'EOF'
[general]
offline = true
EOF

# kylin-doctor 会跳过所有网络相关检测
kylin-doctor scan
```
