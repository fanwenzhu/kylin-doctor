# kylin-doctor 设计文档

**日期**: 2026-05-29
**更新**: 2026-06-02 — 增加云端大模型支持、本地模型资源要求、Ubuntu 20.04 兼容性分析
**状态**: 草稿
**版本**: 1.2

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
| 本地模型 | Ollama + Qwen2.5 | 部署简单，中文效果好，离线可用 |
| 云端模型 | OpenAI 兼容 API | 统一接口适配 Qwen / DeepSeek / Moonshot 等国产供应商 |
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

# 模型切换（全局参数）
kylin-doctor scan --provider local   # 使用本地模型（默认）
kylin-doctor scan --provider cloud   # 使用云端模型
kylin-doctor chat --provider cloud   # 对话时使用云端模型

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
意图识别（LLM 抽象层 → 本地/云端模型）
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
LLM 抽象层生成回答（本地/云端可切换）
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

### 6.4 LLM Provider 抽象层

本地模型和云端模型通过统一的 `LlmProvider` trait 抽象，上层 Agent 无需关心底层实现：

```rust
#[async_trait]
trait LlmProvider: Send + Sync {
    /// 聊天补全（支持 Function Calling）
    async fn chat(&self, messages: &[Message], tools: &[Tool]) -> Result<ChatResponse>;
    /// 文本向量化（RAG 用）
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    /// 供应商名称
    fn name(&self) -> &str;
    /// 是否可用（网络连通性、服务状态检测）
    async fn is_available(&self) -> bool;
}
```

**实现：**
- `OllamaProvider` — 本地模型，默认选择
- `OpenAiCompatProvider` — 通用 OpenAI 兼容接口，适配所有国产云端供应商

### 6.5 云端模型配置

**支持的国产供应商：**

| 供应商 | 推荐模型 | 用途 | API 兼容 |
|--------|---------|------|----------|
| Qwen（阿里云 DashScope） | qwen-plus / qwen-turbo | 通用对话、诊断分析、知识问答 | OpenAI 兼容 |
| DeepSeek | deepseek-chat / deepseek-reasoner | 复杂推理、根因分析 | OpenAI 兼容 |
| Moonshot（Kimi） | moonshot-v1-8k / moonshot-v1-32k | 通用对话、长文档分析 | OpenAI 兼容 |

所有供应商均兼容 OpenAI API 格式，通过 `endpoint` + `api_key` 切换，无需额外适配代码。

**认证方式：**
- API Key 存储在本地配置文件 `~/.kylin-doctor/config.toml`
- 推荐使用环境变量：`QWEN_API_KEY`、`DEEPSEEK_API_KEY`、`MOONSHOT_API_KEY`
- 配置文件中用 `api_key_env = "QWEN_API_KEY"` 引用环境变量，避免明文存储

### 6.6 模型路由策略

**默认策略：local-first（本地优先）**

- 默认使用本地 Ollama 模型，数据不离开本机
- 用户可通过 CLI 参数 `--provider cloud` 显式切换到云端
- 云端不可用（网络断开、API 异常、余额不足）时自动回退本地，并提示用户
- 支持按功能指定不同模型（复杂任务用大模型，简单任务用小模型）

**路由流程：**

```
用户请求
  ↓
检查策略配置（local / cloud / hybrid）
  ↓
┌─ local（默认）→ 本地 Ollama → 失败则报错
├─ cloud        → 云端 API → 失败则回退本地
└─ hybrid       → 本地为主，特定任务（如复杂诊断）用云端
  ↓
返回结果
```

### 6.7 隐私与安全

- **默认离线安全**：local-first 策略下，所有数据仅在本机处理
- **云端切换提示**：切换到云端时明确告知用户"诊断数据将发送至第三方云服务"
- **离线锁定**：配置 `offline = true` 时完全禁止网络请求，即使用户手动指定 `--provider cloud`
- **API Key 保护**：仅存储在本地 `~/.kylin-doctor/config.toml`（权限 600），支持环境变量读取
- **数据最小化**：云端请求仅发送必要的诊断摘要，不发送原始日志或敏感信息

### 6.8 Function Calling 注册

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
offline = false                     # true = 完全禁止网络请求

[llm]
# 策略：local（仅本地）、cloud（仅云端）、hybrid（本地为主，特定任务用云端）
strategy = "local"

[llm.local]
provider = "ollama"
model = "qwen2.5:7b"
embedding_model = "nomic-embed-text"
endpoint = "http://localhost:11434"

[llm.cloud]
provider = "qwen"                  # qwen / deepseek / moonshot
model = "qwen-plus"
api_key_env = "QWEN_API_KEY"       # 从环境变量读取，不明文存储
endpoint = "https://dashscope.aliyuncs.com/compatible-mode/v1"

[llm.cloud.models]
# 按功能指定模型，复杂任务用大模型，简单任务用小模型
diagnosis = "qwen-plus"            # 诊断分析（需要强推理）
chat = "qwen-plus"                 # 对话问答
embedding = "text-embedding-v3"    # 向量化（云端 embedding）
simple_task = "qwen-turbo"         # 意图识别等简单任务

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

## 12. 本地大模型资源要求

端侧设备资源有限，不同模型对硬件的要求差异较大。以下按硬件档位给出推荐配置。

### 12.1 模型规格与资源对照表

| 模型 | 参数量 | 内存(RAM) | 磁盘空间 | CPU 要求 | GPU 加速 | 适用场景 |
|------|--------|-----------|----------|----------|----------|----------|
| qwen2.5:0.5b | 0.5B | ≥2GB | ~400MB | 任意 64 位 CPU | 不需要 | 意图识别、简单问答 |
| qwen2.5:1.5b | 1.5B | ≥4GB | ~1.5GB | 支持 AVX 的双核 | 不需要 | 轻量诊断、低配机器 |
| qwen2.5:7b | 7B | ≥8GB | ~5GB | 支持 AVX2 的四核 | 可选（4GB+ 显存） | 通用诊断分析（推荐） |
| qwen2.5:14b | 14B | ≥16GB | ~9GB | 支持 AVX2 的八核 | 推荐（8GB+ 显存） | 复杂推理、深度分析 |
| nomic-embed-text | 137M | ≥1GB | ~300MB | 任意 64 位 CPU | 不需要 | RAG 文本向量化（必装） |

> **说明**：内存要求为模型加载后的实际占用，系统本身还需预留 1-2GB。磁盘空间为模型文件大小（Ollama 自动管理）。

### 12.2 硬件档位推荐方案

#### 🟢 低配方案（入门级工控机/瘦客户端）

```
硬件：4GB RAM, 双核 CPU, 32GB 存储, 无独立显卡
模型：qwen2.5:0.5b + nomic-embed-text
策略：local 模式，仅做意图识别和简单问答
磁盘占用：~700MB
限制：复杂诊断建议切换云端模型
```

#### 🟡 标准方案（主流工控机/办公电脑）

```
硬件：8GB RAM, 四核 CPU (支持 AVX2), 64GB 存储, 集成显卡
模型：qwen2.5:7b + nomic-embed-text
策略：local 模式，覆盖大部分诊断场景
磁盘占用：~5.3GB
限制：超长上下文分析可能较慢
```

#### 🔴 高配方案（高性能工作站）

```
硬件：16GB+ RAM, 八核 CPU, 128GB+ 存储, 独立显卡 (8GB+ 显存)
模型：qwen2.5:14b + nomic-embed-text
策略：local 模式，全部功能可用，GPU 加速推理
磁盘占用：~9.3GB
限制：无明显限制
```

### 12.3 自动检测与推荐

kylin-doctor 首次运行时自动检测硬件并推荐合适的模型：

```
$ kylin-doctor scan

🔍 检测到首次运行，正在评估硬件...
├── 内存：8GB（可用 6.2GB）
├── CPU：Intel i5-8250U（支持 AVX2）
├── GPU：集成显卡（无 CUDA）
└── 磁盘：/home 剩余 45GB

💡 推荐安装模型：qwen2.5:7b（约 5GB）
   当前使用：qwen2.5:1.5b（低配回退）

   是否安装推荐模型？[Y/n]
```

**自动降级逻辑：**
- RAM < 4GB → 使用 qwen2.5:0.5b，提示切换云端
- RAM 4-8GB → 使用 qwen2.5:1.5b
- RAM ≥ 8GB → 使用 qwen2.5:7b
- RAM ≥ 16GB 且有独显 → 使用 qwen2.5:14b

### 12.4 运行时资源监控

kylin-doctor daemon 模式下持续监控资源使用：

- **内存不足预警**：可用内存 < 1GB 时提示用户关闭其他应用或切换小模型
- **CPU 过载预警**：模型推理时 CPU 持续 100% 超过 30 秒，建议切换小模型
- **磁盘空间预警**：模型存储目录剩余 < 2GB 时提示清理

## 13. 部署兼容性

### 13.1 目标平台

银河麒麟工业桌面操作系统（基于 Ubuntu 20.04 LTS）

| 系统组件 | 版本 |
|----------|------|
| glibc | 2.31 |
| 内核 | 5.4 |
| systemd | 245 |
| Python | 3.8（系统自带） |
| OpenSSL | 1.1.1 |

### 13.2 各组件兼容性分析

| 组件 | 兼容性 | 说明 |
|------|--------|------|
| Rust 核心（CLI + Web 后端） | ✅ 完全兼容 | Rust 最低要求 glibc 2.17 / kernel 3.2，远超 Ubuntu 20.04 提供的版本 |
| LanceDB（向量数据库） | ✅ 兼容 | Rust crate 从源码编译，glibc 2.31 满足要求 |
| Vue 3 前端 | ✅ 兼容 | 运行在浏览器中，无系统依赖 |
| sysinfo（系统检测） | ✅ 兼容 | 读取 /proc 和 /sys，kernel 5.4 完全支持 |
| tokio / axum（异步运行时） | ✅ 兼容 | 使用 epoll，kernel 2.6+ 即可 |
| Ollama（本地大模型） | ⚠️ **有风险** | 见下文详细说明 |

### 13.3 Ollama 兼容性问题

**问题描述：**

Ollama 的预编译二进制捆绑了 GPU 计算库（CUDA/ROCm），这些库使用较新工具链编译，可能依赖 glibc 2.32+ 的符号（如 `pthread_attr_setaffinity_np@@GLIBC_2.32`）。在 Ubuntu 20.04（glibc 2.31）上运行可能报错：

```
ollama: /lib/x86_64-linux-gnu/libc.so.6: version `GLIBC_2.32' not found
```

Ollama 官方未发布 Ubuntu 20.04 的支持声明，近期版本在此平台上运行的风险持续增大。

**解决方案（按推荐度排序）：**

#### 方案一：Docker 部署 Ollama（推荐）

Docker 镜像自带 glibc，完全绕过宿主机版本限制：

```bash
# 安装 Ollama Docker 镜像
docker run -d \
  --name ollama \
  --restart unless-stopped \
  --gpus=all \
  -v ollama-data:/root/.ollama \
  -p 11434:11434 \
  ollama/ollama

# 拉取模型
docker exec ollama ollama pull qwen2.5:7b
docker exec ollama ollama pull nomic-embed-text
```

kylin-doctor 配置中将 endpoint 指向 Docker 容器即可：

```toml
[llm.local]
endpoint = "http://localhost:11434"  # Docker 暴露的端口，与原生安装无差异
```

**优点**：无需编译、自动更新、GPU 直通
**缺点**：需要安装 Docker、首次拉取镜像约 1GB

#### 方案二：Rust 静态编译（kylin-doctor 本身）

kylin-doctor 自身使用 musl target 编译完全静态链接的二进制，消除 glibc 依赖：

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

**优点**：二进制无依赖，可直接分发
**缺点**：仅解决 kylin-doctor 自身，不影响 Ollama；C 依赖库需额外配置

#### 方案三：云端模型兜底

当本地 Ollama 无法运行时，自动切换到国产云端模型（Qwen / DeepSeek / Moonshot）。已有 `local-first` 策略支持此场景。

**优点**：零本地资源占用、模型能力最强
**缺点**：需要网络、数据发送至云端

### 13.4 部署建议

对于银河麒麟工业桌面操作系统，推荐的部署组合：

| 场景 | 本地模型部署方式 | 模型选择 | 备注 |
|------|-----------------|---------|------|
| 有 Docker 环境 | Docker Ollama | qwen2.5:7b | 最佳方案，GPU 直通 |
| 无 Docker，可联网 | 不安装本地模型 | 云端模型 | 零资源占用 |
| 无 Docker，离线环境 | 源码编译 Ollama | qwen2.5:1.5b | 需验证 glibc 兼容性 |
| 资源极有限 | 不安装本地模型 | 云端模型或纯规则引擎 | 4GB 以下 RAM |

### 13.5 kylin-doctor 自身的系统要求

| 组件 | 最低要求 | 推荐配置 |
|------|---------|---------|
| CPU | 64 位双核 | 四核（支持 AVX2） |
| RAM | 512MB（不含模型） | 1GB+ |
| 磁盘 | 50MB（二进制）+ 模型空间 | 20GB+ |
| 网络 | 可选（云端模型需要） | 可选 |
| 依赖 | 无（静态编译） | Docker（本地模型用） |
