# kylin-doctor

银河麒麟桌面系统自我诊断工具。

## 功能特性

- **全面诊断** — 硬件、系统、软件、安全、性能五大维度
- **分层输出** — 普通用户 / 管理员 / 开发者三种详细程度
- **AI 增强** — 接入本地大模型（Ollama），智能分析问题根因
- **自动修复** — 检测到问题后自动修复，执行前确认
- **双界面** — CLI 命令行 + Web 仪表盘

## 快速开始

### 构建

```bash
cargo build --release
```

### 使用

```bash
# 全面扫描
kylin-doctor scan

# 只扫描系统模块
kylin-doctor scan --module system

# 详细输出（开发者模式）
kylin-doctor scan --verbose 2

# 简要输出（普通用户模式）
kylin-doctor scan --verbose 0
```

### 输出示例

```
🔍 kylin-doctor 系统诊断

├── [████████████████████] 1/1 正在扫描 system...
└── 扫描完成

📋 system [⚠️  2 个警告]
   扫描耗时: 12ms
   ⚠️  [system-disk-warning-_home] /home 磁盘空间偏高 (85%)
      💡 建议: 清理临时文件

   ⚠️  [system-zombie-processes] 发现 2 个僵尸进程

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
⚠️  警告: 2  ℹ️  信息: 0
系统基本正常，但有警告项需要关注。
```

## 项目结构

```
kylin-doctor/
├── Cargo.toml
├── crates/
│   ├── kylin-doctor-core/    # 核心检测逻辑
│   └── kylin-doctor-cli/     # CLI 命令行工具
├── web/                      # Web 仪表盘（规划中）
└── docs/                     # 设计文档
```

## 技术栈

| 组件 | 技术 |
|------|------|
| CLI 框架 | clap (Rust) |
| Web 后端 | Axum (规划中) |
| Web 前端 | Vue 3 + Element Plus (规划中) |
| 本地模型 | Ollama + Qwen2.5 (规划中) |
| 系统检测 | Shell 命令 + /proc |

## 路线图

- [x] 第一阶段：项目骨架 + 系统检测模块 + CLI scan 命令
- [ ] 第二阶段：硬件检测模块 (hardware.rs)
- [ ] 第三阶段：软件生态模块 (software.rs)
- [ ] 第四阶段：安全审计模块 (security.rs)
- [ ] 第五阶段：性能分析模块 (performance.rs)
- [ ] 第六阶段：AI 集成 (Ollama + RAG)
- [ ] 第七阶段：Web 仪表盘

## License

MIT
