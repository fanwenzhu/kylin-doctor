# kylin-doctor 设计文档

**日期**: 2026-05-29
**状态**: 草稿
**版本**: 1.0

## 1. 概述

kylin-doctor 是一个银河麒麟桌面系统的自我诊断工具，面向多种用户（普通用户、管理员、开发者），提供全面的系统检测、智能分析和自动修复功能。

### 1.1 核心特性

- **全面诊断**: 硬件、系统、软件、安全、性能五大维度
- **分层设计**: 不同用户看到不同深度的信息
- **AI 增强**: 接入本地大模型（Ollama），智能分析问题根因
- **自动修复**: 检测到问题后自动修复，执行前确认
- **双界面**: CLI 命令行 + Web 仪表盘

### 1.2 目标用户

| 用户类型 | 使用方式 | 信息深度 |
|---------|---------|---------|
| 普通终端用户 | Web 仪表盘、简单 CLI | 只显示问题和建议 |
| 系统管理员 | CLI、Web 仪表盘 | 显示检测项名称和结果 |
| 开发者/技术支持 | CLI、深度诊断 | 显示完整技术细节 |

## 2. 技术栈

| 组件 | 技术 | 理由 |
|------|------|------|
| CLI 框架 | clap (Rust) | 功能全，社区活跃 |
| Web 后端 | Axum | 轻量，异步，Tokio 生态 |
| Web 前端 | Vue 3 + Element Plus | 国产 UI 库，中文友好 |
| 图表 | ECharts | 功能强，中文文档全 |
| 向量数据库 | lancedb | 纯 Rust，嵌入式，无需额外服务 |
| 本地模型 | Ollama + Qwen2.5 | 部署简单，中文效果好 |
| 系统检测 | sysinfo + 系统命令调用 | Rust 原生 + shell 互补 |
| 配置管理 | TOML | 简洁，Rust 原生支持 |
| 报告生成 | HTML + Askama 模板 | 美观，可离线查看 |

## 3. 项目结构

```
kylin-doctor/
├── Cargo.toml                    # Rust 工作空间
├── crates/
│   ├── kylin-doctor-cli/         # CLI 主程序
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs           # 入口
│   │       ├── commands/         # 子命令（scan, report, chat）
│   │       ├── detectors/        # 检测模块
│   │       │   ├── mod.rs
│   │       │   ├── hardware.rs   # 硬件检测
│   │       │   ├── system.rs     # 系统健康
│   │       │   ├── software.rs   # 软件生态
│   │       │   ├── security.rs   # 安全审计
│   │       │   └── performance.rs# 性能分析
│   │       ├── llm/              # Ollama 集成
│   │       └── report/           # 报告生成
│   └── kylin-doctor-core/        # 核心检测逻辑（CLI/Web 共用）
│       ├── Cargo.toml
│       └── src/
├── web/                          # Web 仪表盘
│   ├── Cargo.toml                # Axum 后端
│   ├── src/
│   │   ├── main.rs
│   │   ├── api/                  # REST API
│   │   └── ws/                   # WebSocket（实时推送诊断状态）
│   └── frontend/                 # Vue 3 前端
│       ├── package.json
│       └── src/
├── docs/
│   └── superpowers/specs/        # 设计文档
└── README.md
```

### 3.1 关键设计决策

- `kylin-doctor-core` 独立出来，CLI 和 Web 共用同一套检测逻辑，避免重复代码
- `kylin-doctor-cli` 依赖 `kylin-doctor-core`，负责命令行解析和用户交互
- `web/` 后端同样依赖 `kylin-doctor-core`，提供 REST API 和 WebSocket
- `detectors/` 按诊断领域分模块，每个模块独立，方便扩展
- Web 前端用 Vue 3 + Element Plus（国产 UI 库，适配中文）

## 4. CLI 命令设计

### 4.1 主要命令

```bash
# 全面扫描
kylin-doctor scan                    # 全面扫描
kylin-doctor scan --module hardware  # 只扫描硬件
kylin-doctor scan --module system    # 只扫描系统
kylin-doctor scan --quick            # 快速扫描（跳过耗时项）

# 诊断报告
kylin-doctor report                  # 生成完整报告
kylin-doctor report --format html    # HTML 格式
kylin-doctor report --format json    # JSON 格式（便于程序处理）
kylin-doctor report --output /tmp/   # 指定输出目录

# 智能问答
kylin-doctor chat                    # 进入交互式对话
kylin-doctor ask "为什么我的WiFi连不上？"  # 单次提问

# Web 仪表盘
kylin-doctor serve                   # 启动 Web 服务（默认 127.0.0.1:8080）
kylin-doctor serve --port 9090       # 指定端口

# 系统巡检（定时任务）
kylin-doctor daemon                  # 后台运行，定时巡检
kylin-doctor daemon --interval 3600  # 每小时巡检一次

# 诊断 + 修复
kylin-doctor scan --fix              # 扫描后自动修复所有问题
kylin-doctor fix                     # 直接修复上次扫描发现的问题
kylin-doctor fix --issue 001         # 只修复指定问题
kylin-doctor fix --dry-run           # 预览修复操作，不实际执行
kylin-doctor fix --yes               # 跳过确认，直接修复（危险模式）

# 知识库管理
kylin-doctor knowledge add ./docs/麒麟管理手册.pdf
kylin-doctor knowledge add ./docs/ --recursive
kylin-doctor knowledge status
kylin-doctor knowledge test "如何配置打印机"
```

### 4.2 输出分层设计

- `--verbose 0`：只显示问题和建议（普通用户）
- `--verbose 1`：显示检测项名称和结果（管理员）
- `--verbose 2`：显示完整技术细节（开发者/技术支持）

### 4.3 退出码

- `0`：一切正常
- `1`：有警告
- `2`：有严重问题
- `10`：工具自身出错

## 5. 检测模块设计

### 5.1 统一接口

```rust
trait Detector {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn scan(&self) -> Vec<Finding>;           // 执行检测
    fn fix(&self, finding: &Finding) -> Result<FixResult>;  // 修复
    fn is_available(&self) -> bool;           // 模块是否可用
}
```

### 5.2 检测结果结构

```rust
struct Finding {
    id: String,              // 唯一标识
    module: String,          // 所属模块
    severity: Severity,      // info/warning/critical
    title: String,           // 简短标题
    description: String,     // 详细描述
    evidence: String,        // 检测证据（命令输出、日志片段）
    fix: Option<FixAction>,  // 修复方案
    auto_fixable: bool,      // 是否可自动修复
}
```

### 5.3 硬件检测模块 (hardware.rs)

**检测项：**
- CPU — 型号、核心数、温度、频率、异常计数
- 内存 — 容量、使用率、ECC 状态、坏块检测
- 磁盘 — SMART 健康、坏道、读写速度、寿命
- 网卡 — 连接状态、速度、丢包率、驱动版本
- 显卡 — 型号、驱动、显存、温度
- 外设 — USB 设备、打印机、扫描仪状态
- 主板 — BIOS 版本、电池电压、传感器数据

**修复能力：**
- 更新驱动
- 调整硬件参数（如 CPU 调频策略）
- 标记故障硬件并建议更换

### 5.4 系统健康模块 (system.rs)

**检测项：**
- 服务状态 — systemd 服务是否正常运行
- 进程异常 — 僵尸进程、CPU/内存大户
- 磁盘空间 — 各分区使用率、大文件定位
- 日志分析 — /var/log 关键错误、OOM 记录
- 启动分析 — 启动耗时、慢服务定位
- 内核状态 — 版本、加载模块、dmesg 异常

**修复能力：**
- 重启异常服务
- 清理临时文件和日志
- 杀掉僵尸进程
- 优化启动服务

### 5.5 软件生态模块 (software.rs)

**检测项：**
- 包管理 — 源配置、可更新包、损坏包
- 依赖冲突 — 依赖树分析、版本冲突
- 应用兼容 — Windows 兼容层(wine)、Android 兼容层
- 运行时 — Java/Python/Node 版本和环境
- 字体 — 中文字体完整性、渲染异常

**修复能力：**
- 修复损坏的包
- 解决依赖冲突
- 更新软件源
- 安装缺失的中文字体

### 5.6 安全审计模块 (security.rs)

**检测项：**
- 用户账户 — 空密码、过期账户、sudo 权限
- 文件权限 — 关键目录权限、SUID 文件
- 防火墙 — 状态、规则、开放端口
- SSH 配置 — root 登录、密码认证、端口
- 漏洞扫描 — CVE 匹配、已知漏洞
- 审计日志 — 登录记录、异常操作

**修复能力：**
- 锁定危险账户
- 修复文件权限
- 加固 SSH 配置
- 安装安全补丁

### 5.7 性能分析模块 (performance.rs)

**检测项：**
- CPU — 使用率、负载、调度延迟
- 内存 — 使用趋势、碎片率、缓存效率
- 磁盘 I/O — 读写延迟、队列深度、IOPS
- 网络 — 带宽、延迟、连接数
- 桌面 — 合成器帧率、窗口响应延迟
- 电源 — 电池健康、功耗模式

**修复能力：**
- 调整 I/O 调度器
- 优化内存参数
- 调整 CPU 调频策略
- 清理桌面合成器缓存

## 6. AI 集成设计

### 6.1 架构概览

```
用户输入
  ↓
意图识别（本地模型）
  ↓
┌─────────┐    ┌─────────┐    ┌─────────┐
│ 诊断    │    │ 知识    │    │ 修复    │
│ Agent   │    │ 问答    │    │ Agent   │
└────┬────┘    └────┬────┘    └────┬────┘
     ↓              ↓              ↓
┌─────────────────────────────────────────┐
│           Tool Registry                │
│  scan.hardware()  scan.system()        │
│  scan.software()  scan.security()      │
│  scan.performance()  fix.issue()       │
└─────────────────────────────────────────┘
     ↓              ↓              ↓
┌─────────────────────────────────────────┐
│        RAG 知识库（麒麟文档）            │
│  向量检索 → 相关文档片段                 │
└─────────────────────────────────────────┘
     ↓
本地大模型生成回答
  ↓
输出（文本 + 修复方案 + 操作确认）
```

### 6.2 三个 AI 角色

| 角色 | 职责 | 调用工具 |
|------|------|----------|
| 诊断 Agent | 分析问题根因 | 调用 5 个检测模块 |
| 知识问答 | 回答麒麟系统问题 | 检索 RAG 知识库 |
| 修复 Agent | 制定和执行修复方案 | 调用修复工具 |

### 6.3 RAG 知识库

**知识库结构：**
```
~/.kylin-doctor/knowledge/
├── vector_db/              # 向量数据库（lancedb）
│   └── kylin_knowledge     # 麒麟知识集合
├── raw_docs/               # 原始文档
│   ├── official/           # 官方文档
│   ├── faq/                # 常见问题
│   ├── known_issues/       # 已知问题
│   └── hardware_compat/    # 硬件兼容性
└── ingest.sh               # 文档导入脚本
```

**知识库集成流程：**
1. 用户通过 `kylin-doctor knowledge add` 导入文档
2. 文档被分块并向量化，存入 lancedb
3. AI 对话时，先检索相关文档片段
4. 将检索到的文档片段与用户问题一起喂给大模型
5. 大模型基于文档内容生成回答

**本地模型选择：**
```bash
ollama pull qwen2.5:7b        # 通用对话，效果好
ollama pull qwen2.5:1.5b      # 轻量版，低配机器用
ollama pull nomic-embed-text   # 文本向量化（RAG 用）
```

### 6.4 Function Calling 注册

```rust
let tools = vec![
    Tool {
        name: "scan_hardware",
        description: "扫描硬件状态，包括CPU、内存、磁盘、网卡等",
        parameters: json!({
            "component": "cpu|memory|disk|network|gpu|all"
        }),
    },
    Tool {
        name: "scan_system",
        description: "扫描系统健康状态，包括服务、进程、日志等",
        parameters: json!({}),
    },
    Tool {
        name: "fix_issue",
        description: "修复指定问题",
        parameters: json!({
            "issue_id": "问题ID",
            "confirm": true
        }),
    },
    // ... 其他工具
];
```

## 7. Web 仪表盘设计

### 7.1 界面布局

```
┌─────────────────────────────────────────────────┐
│  kylin-doctor Web Dashboard                     │
├──────────┬──────────────────────────────────────┤
│          │                                      │
│  侧边栏   │   主内容区                           │
│          │                                      │
│  📊 总览  │   ┌──────────────────────────────┐  │
│  🔧 硬件  │   │  系统健康度：87/100           │  │
│  💻 系统  │   │  ████████████░░░  87%         │  │
│  📦 软件  │   └──────────────────────────────┘  │
│  🔒 安全  │                                      │
│  ⚡ 性能  │   ┌────────┐ ┌────────┐ ┌────────┐  │
│  🤖 AI   │   │ 硬件   │ │ 系统   │ │ 安全   │  │
│  📋 报告  │   │ ✅正常  │ │ ⚠️2警告│ │ 🔴1严重│  │
│          │   └────────┘ └────────┘ └────────┘  │
│          │                                      │
│          │   最近问题：                          │
│          │   • /var/log 空间不足 (89%)           │
│          │   • sshd 服务异常重启 3 次            │
│          │   • 内核模块 nouveau 性能不佳          │
│          │                                      │
│          │   [一键扫描]  [查看详情]  [生成报告]   │
└──────────┴──────────────────────────────────────┘
```

### 7.2 功能特性

- **实时状态** — WebSocket 推送，扫描过程中实时更新进度和结果
- **可视化图表** — 硬件温度、CPU/内存趋势、磁盘使用等
- **交互式诊断** — 点击问题项查看详细信息和修复方案
- **AI 对话** — 内嵌聊天窗口，直接和大模型对话
- **报告导出** — 一键生成 PDF/HTML 报告

### 7.3 技术实现

- 后端：Axum（Rust）
- 前端：Vue 3 + Element Plus + ECharts
- 通信：REST API + WebSocket

## 8. 修复流程设计

### 8.1 修复流程

```
扫描 → 发现问题 → 生成修复方案 → 用户确认 → 执行修复 → 验证结果
```

### 8.2 修复方案展示

每个修复方案包含：
- 问题描述
- 修复命令（用户能看到具体做什么）
- 风险等级（低/中/高）
- 预期效果

### 8.3 执行后验证

- 修复完重新检测该项，确认问题是否解决
- 如果没解决，给出进一步建议

## 9. 数据存储

### 9.1 存储位置

```
~/.kylin-doctor/
├── scans/          # 历史扫描记录
├── reports/        # 生成的报告
├── knowledge/      # RAG 知识库
└── config.toml     # 配置文件
```

### 9.2 配置文件示例

```toml
[general]
verbose = 1
auto_fix = false
confirm_before_fix = true

[ollama]
model = "qwen2.5:7b"
embedding_model = "nomic-embed-text"
endpoint = "http://localhost:11434"

[web]
host = "127.0.0.1"
port = 8080

[daemon]
interval = 3600
notify = true
```

## 10. 定时巡检模式

```bash
# 启动后台守护进程
kylin-doctor daemon --interval 3600 --notify

# 每小时自动巡检，发现问题桌面通知
# 通知内容：
# ⚠️ 麒麟医生发现 2 个问题需要关注
# - 磁盘 /home 使用率 92%
# - 内核安全更新待安装
# [查看详情] [立即修复]
```

## 11. 使用场景示例

### 11.1 普通用户一键体检

```
$ kylin-doctor scan --fix

🔍 开始全面体检...
├── [1/5] 硬件检测    ████████████████████ 100%  ✅ 正常
├── [2/5] 系统健康    ████████████████████ 100%  ⚠️ 2 个警告
├── [3/5] 软件生态    ████████████████████ 100%  ✅ 正常
├── [4/5] 安全审计    ████████████████████ 100%  🔴 1 个严重
└── [5/5] 性能分析    ████████████████████ 100%  ✅ 正常

📋 体检报告：
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🔴 严重：SSH 允许 root 密码登录
⚠️ 警告：/var/log 空间使用 89%
⚠️ 警告：3 个软件包可更新

是否自动修复以上问题？[Y/n] y

修复中...
✅ 已加固 SSH 配置（禁止 root 密码登录）
✅ 已清理 30 天前的日志，释放 2.1GB
✅ 已更新 3 个软件包

验证修复结果...
✅ 全部修复成功！系统健康度：95/100
```

### 11.2 AI 对话诊断

```
$ kylin-doctor chat

🤖 麒麟医生 AI 助手已就绪，有什么可以帮您？

用户：我插入U盘后文件管理器没有反应

AI：我来检查一下 USB 相关的状态...

    [正在扫描 USB 子系统...]
    
    检测结果：
    - USB 控制器：正常（xHCI）
    - U 盘已识别：Kingston DataTraveler 32GB (/dev/sdb1)
    - 文件系统：FAT32，正常挂载到 /media/user/KINGSTON
    
    问题定位：文件管理器的自动挂载通知服务未运行
    
    修复方案：重启 udisks2 服务
    命令：sudo systemctl restart udisks2
    
    是否执行？

用户：是

AI：正在执行...
    ✅ udisks2 服务已重启
    ✅ U 盘已重新挂载
    
    现在文件管理器应该能正常显示了。
    如果还有问题，可能需要检查桌面环境的自动挂载设置。
```
