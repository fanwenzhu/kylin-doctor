use crate::detector::{Detector, Finding, FixAction, ScanReport, Severity};
use crate::util::{command_output_with_timeout, DEFAULT_CMD_TIMEOUT_SECS, LONG_CMD_TIMEOUT_SECS};
use std::process::Command;
use std::time::{Duration, Instant};

/// 软件生态检测模块
pub struct SoftwareDetector;

impl SoftwareDetector {
    pub fn new() -> Self {
        Self
    }

    /// 检查 APT 包管理器状态
    fn check_apt_status(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        let timeout = Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS);

        // 检查是否有损坏的包
        let output = match command_output_with_timeout(
            Command::new("dpkg").args(["--audit"]),
            timeout,
        ) {
            Some(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            None => return findings,
        };

        if !output.trim().is_empty() {
            findings.push(Finding {
                id: "sw-pkg-broken".to_string(),
                module: "software".to_string(),
                severity: Severity::Warning,
                title: "存在损坏或不完整的软件包".to_string(),
                description: format!(
                    "dpkg 审计发现异常包，可能导致软件无法正常运行。"
                ),
                evidence: output.lines().take(10).collect::<Vec<_>>().join("\n"),
                fix: Some(FixAction {
                    description: "修复损坏的包".to_string(),
                    command: "sudo dpkg --configure -a && sudo apt-get install -f".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: true,
            });
        }

        // 检查是否有可更新的包（apt 可能因锁阻塞，用较长超时）
        let upgrade_output = match command_output_with_timeout(
            Command::new("apt").args(["list", "--upgradable"]),
            Duration::from_secs(LONG_CMD_TIMEOUT_SECS),
        ) {
            Some(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            None => return findings,
        };

        let upgradable_count = upgrade_output
            .lines()
            .skip(1) // 跳过 "Listing..." 标题行
            .filter(|l| !l.trim().is_empty())
            .count();

        if upgradable_count > 0 {
            let severity = if upgradable_count > 50 {
                Severity::Warning
            } else {
                Severity::Info
            };

            findings.push(Finding {
                id: "sw-pkg-upgradable".to_string(),
                module: "software".to_string(),
                severity,
                title: format!("{} 个软件包可更新", upgradable_count),
                description: format!(
                    "系统中有 {} 个软件包有可用更新，建议定期更新以获取安全补丁和新功能。",
                    upgradable_count
                ),
                evidence: upgrade_output
                    .lines()
                    .skip(1)
                    .take(10)
                    .collect::<Vec<_>>()
                    .join("\n"),
                fix: Some(FixAction {
                    description: "更新所有软件包".to_string(),
                    command: "sudo apt-get update && sudo apt-get upgrade -y".to_string(),
                    risk_level: "medium".to_string(),
                    ..Default::default()
                }),
                auto_fixable: true,
            });
        }

        findings
    }

    /// 检查 APT 软件源配置
    fn check_apt_sources(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 检查 sources.list 是否存在
        let sources_content = match std::fs::read_to_string("/etc/apt/sources.list") {
            Ok(c) => c,
            Err(_) => return findings,
        };

        // 检查是否所有源都被注释掉了
        let active_sources: Vec<&str> = sources_content
            .lines()
            .filter(|l| {
                let trimmed = l.trim();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
            .collect();

        if active_sources.is_empty() {
            findings.push(Finding {
                id: "sw-apt-no-sources".to_string(),
                module: "software".to_string(),
                severity: Severity::Critical,
                title: "APT 软件源全部被注释".to_string(),
                description: "/etc/apt/sources.list 中没有活跃的软件源，无法安装或更新软件。".to_string(),
                evidence: sources_content
                    .lines()
                    .take(10)
                    .collect::<Vec<_>>()
                    .join("\n"),
                fix: Some(FixAction {
                    description: "恢复默认软件源".to_string(),
                    command: "echo '请检查 /etc/apt/sources.list 并恢复正确的软件源配置'".to_string(),
                    risk_level: "medium".to_string(),
                    ..Default::default()
                }),
                auto_fixable: false,
            });
        }

        // 检查 apt update 是否正常（apt 可能因锁阻塞，用较长超时）
        let update_output = command_output_with_timeout(
            Command::new("apt-get").args(["update", "--dry-run"]),
            Duration::from_secs(LONG_CMD_TIMEOUT_SECS),
        );

        if let Some(o) = update_output {
            let stderr = String::from_utf8_lossy(&o.stderr);
            if stderr.contains("NO_PUBKEY") || stderr.contains("EXPKEYSIG") {
                findings.push(Finding {
                    id: "sw-apt-key-error".to_string(),
                    module: "software".to_string(),
                    severity: Severity::Warning,
                    title: "APT 软件源签名密钥异常".to_string(),
                    description: "部分软件源的 GPG 密钥缺失或过期，可能导致无法安全下载软件包。".to_string(),
                    evidence: stderr
                        .lines()
                        .filter(|l| l.contains("NO_PUBKEY") || l.contains("EXPKEYSIG"))
                        .take(5)
                        .collect::<Vec<_>>()
                        .join("\n"),
                    fix: Some(FixAction {
                        description: "更新软件源密钥".to_string(),
                        command: "sudo apt-key adv --keyserver keyserver.ubuntu.com --recv-keys $(apt-key list 2>/dev/null | grep -A1 'expired' | grep -oP '[0-9A-F]{16}')".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            }
        }

        findings
    }

    /// 检查运行时环境
    fn check_runtimes(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 检查 Python
        let python_check = check_command_version("python3", &["--version"]);
        if python_check.is_none() {
            findings.push(Finding {
                id: "sw-no-python3".to_string(),
                module: "software".to_string(),
                severity: Severity::Info,
                title: "未检测到 Python 3".to_string(),
                description: "系统未安装 Python 3，部分系统工具和应用可能依赖 Python。".to_string(),
                evidence: "python3 not found".to_string(),
                fix: Some(FixAction {
                    description: "安装 Python 3".to_string(),
                    command: "sudo apt-get install -y python3".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: true,
            });
        }

        // 检查 Java
        let _java_check = check_command_version("java", &["-version"]);
        // Java 不是必须的，只做信息提示

        // 检查 Node.js
        let _node_check = check_command_version("node", &["--version"]);

        // 检查 pip 是否可用（如果 Python 存在）
        if python_check.is_some() {
            let pip_check = check_command_version("pip3", &["--version"]);
            if pip_check.is_none() {
                findings.push(Finding {
                    id: "sw-no-pip3".to_string(),
                    module: "software".to_string(),
                    severity: Severity::Info,
                    title: "未检测到 pip3".to_string(),
                    description: "Python 3 已安装但 pip3 不可用，无法管理 Python 第三方包。".to_string(),
                    evidence: "pip3 not found".to_string(),
                    fix: Some(FixAction {
                        description: "安装 pip3".to_string(),
                        command: "sudo apt-get install -y python3-pip".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        findings
    }

    /// 检查中文字体
    fn check_chinese_fonts(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        let timeout = Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS);

        // 通过 fc-list 检查中文字体
        let output = match command_output_with_timeout(
            Command::new("fc-list").args([":lang=zh"]),
            timeout,
        ) {
            Some(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            None => {
                // fc-list 不可用，尝试直接检查字体目录
                return self.check_font_dirs();
            }
        };

        if output.trim().is_empty() {
            findings.push(Finding {
                id: "sw-no-chinese-font".to_string(),
                module: "software".to_string(),
                severity: Severity::Warning,
                title: "未检测到中文字体".to_string(),
                description: "系统中没有安装中文字体，中文内容可能显示为方块或乱码。".to_string(),
                evidence: "fc-list :lang=zh returned empty".to_string(),
                fix: Some(FixAction {
                    description: "安装中文字体".to_string(),
                    command: "sudo apt-get install -y fonts-wqy-zenhei fonts-wqy-microhei".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: true,
            });
        }

        findings
    }

    /// 备用：直接检查字体目录
    fn check_font_dirs(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        let timeout = Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS);

        let font_dirs = ["/usr/share/fonts", "/usr/local/share/fonts"];
        let mut found_chinese = false;

        for dir in &font_dirs {
            if let Some(find_output) = command_output_with_timeout(
                Command::new("find")
                    .args([dir, "-name", "*wqy*", "-o", "-name", "*noto*cjk*", "-o", "-name", "*simhei*", "-o", "-name", "*simsun*"]),
                timeout,
            ) {
                let stdout = String::from_utf8_lossy(&find_output.stdout);
                if !stdout.trim().is_empty() {
                    found_chinese = true;
                    break;
                }
            }
        }

        if !found_chinese {
            findings.push(Finding {
                id: "sw-no-chinese-font".to_string(),
                module: "software".to_string(),
                severity: Severity::Warning,
                title: "未检测到中文字体".to_string(),
                description: "字体目录中未找到常见中文字体文件，中文显示可能异常。".to_string(),
                evidence: "searched /usr/share/fonts and /usr/local/share/fonts".to_string(),
                fix: Some(FixAction {
                    description: "安装中文字体".to_string(),
                    command: "sudo apt-get install -y fonts-wqy-zenhei fonts-wqy-microhei".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: true,
            });
        }

        findings
    }

    /// 检查 Wine 兼容层
    fn check_wine(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 检查 wine 是否安装
        let wine_installed = check_command_version("wine", &["--version"]).is_some();

        if wine_installed {
            // 检查 wine 前缀是否存在且完整
            let home = std::env::var("HOME").unwrap_or_default();
            let wine_prefix = format!("{}/.wine", home);

            if !std::path::Path::new(&wine_prefix).exists() {
                findings.push(Finding {
                    id: "sw-wine-no-prefix".to_string(),
                    module: "software".to_string(),
                    severity: Severity::Info,
                    title: "Wine 已安装但未初始化".to_string(),
                    description: "Wine 已安装但 ~/.wine 目录不存在，首次运行 Windows 程序时会自动创建。".to_string(),
                    evidence: format!("{} not found", wine_prefix),
                    fix: Some(FixAction {
                        description: "初始化 Wine 环境".to_string(),
                        command: "wineboot --init".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        findings
    }

    /// 检查依赖冲突
    fn check_dependency_conflicts(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        let timeout = Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS);

        // apt-get check 检查依赖完整性
        let check_output = command_output_with_timeout(
            Command::new("apt-get").args(["check"]),
            timeout,
        );

        if let Some(o) = check_output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let stderr = String::from_utf8_lossy(&o.stderr);
            let combined = format!("{}\n{}", stdout, stderr);

            // 正常输出是 "Reading package lists... Done\nBuilding dependency tree\nReading state information... Done\n"
            // 有问题时会输出具体错误
            let problems: Vec<&str> = combined
                .lines()
                .filter(|l| {
                    l.contains("broken")
                        || l.contains("depends")
                        || l.contains("not installed")
                        || l.contains("has no candidate")
                        || l.contains("error")
                })
                .collect();

            if !problems.is_empty() {
                findings.push(Finding {
                    id: "sw-dep-conflict".to_string(),
                    module: "software".to_string(),
                    severity: Severity::Warning,
                    title: "存在依赖冲突或损坏的包".to_string(),
                    description: format!(
                        "apt-get check 发现 {} 个依赖问题，部分软件可能无法正常运行。",
                        problems.len()
                    ),
                    evidence: problems.iter().take(10).map(|s| s.to_string()).collect::<Vec<_>>().join("\n"),
                    fix: Some(FixAction {
                        description: "修复依赖关系".to_string(),
                        command: "sudo apt-get install -f && sudo dpkg --configure -a".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        // 检查 held（被锁定）的包
        let held_output = command_output_with_timeout(
            Command::new("dpkg").args(["--get-selections"]),
            timeout,
        );

        if let Some(o) = held_output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let held_packages: Vec<&str> = stdout
                .lines()
                .filter(|l| l.contains("hold"))
                .collect();

            if !held_packages.is_empty() {
                findings.push(Finding {
                    id: "sw-pkg-held".to_string(),
                    module: "software".to_string(),
                    severity: Severity::Info,
                    title: format!("{} 个包被锁定 (hold)", held_packages.len()),
                    description: format!(
                        "以下软件包被标记为 hold，不会被 apt upgrade 自动更新。{}",
                        if held_packages.len() > 5 { "（仅显示前 5 个）" } else { "" }
                    ),
                    evidence: held_packages
                        .iter()
                        .take(5)
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                    fix: Some(FixAction {
                        description: "解除包锁定".to_string(),
                        command: "echo '使用 sudo apt-mark unhold <包名> 解除锁定'".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            }
        }

        // 检查残留的配置文件（已卸载但配置未清理）
        let residual_output = command_output_with_timeout(
            Command::new("dpkg").args(["-l"]),
            timeout,
        );

        if let Some(o) = residual_output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let residual_count = stdout
                .lines()
                .filter(|l| l.starts_with("rc"))
                .count();

            if residual_count > 20 {
                findings.push(Finding {
                    id: "sw-pkg-residual".to_string(),
                    module: "software".to_string(),
                    severity: Severity::Info,
                    title: format!("{} 个已卸载软件残留配置文件", residual_count),
                    description: format!(
                        "有 {} 个已卸载的软件包残留了配置文件（dpkg 状态 rc），可清理释放空间。",
                        residual_count
                    ),
                    evidence: format!("residual_config_packages={}", residual_count),
                    fix: Some(FixAction {
                        description: "清理残留配置文件".to_string(),
                        command: "sudo dpkg -l | grep '^rc' | awk '{print $2}' | xargs sudo dpkg --purge".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        findings
    }

    /// 检查 Android 兼容层
    fn check_android_compat(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 检查 Anbox
        let anbox_installed = check_command_version("anbox", &["version"]).is_some()
            || std::path::Path::new("/snap/anbox").exists();

        if anbox_installed {
            let anbox_running = Command::new("pgrep")
                .args(["-x", "anbox"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if !anbox_running {
                findings.push(Finding {
                    id: "sw-anbox-not-running".to_string(),
                    module: "software".to_string(),
                    severity: Severity::Info,
                    title: "Anbox 已安装但未运行".to_string(),
                    description: "Anbox (Android 兼容层) 已安装但进程未启动。".to_string(),
                    evidence: "anbox process not found".to_string(),
                    fix: Some(FixAction {
                        description: "启动 Anbox".to_string(),
                        command: "anbox launch --package=org.anbox.appmgr --component=org.anbox.appmgr.AppViewActivity".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            }
        }

        // 检查 Waydroid
        let waydroid_installed = check_command_version("waydroid", &["--version"]).is_some();

        if waydroid_installed {
            let waydroid_status = command_output_with_timeout(
                Command::new("waydroid").args(["status"]),
                Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS),
            );

            if let Some(o) = waydroid_status {
                let stdout = String::from_utf8_lossy(&o.stdout);
                if stdout.contains("not running") || stdout.contains("STOPPED") {
                    findings.push(Finding {
                        id: "sw-waydroid-not-running".to_string(),
                        module: "software".to_string(),
                        severity: Severity::Info,
                        title: "Waydroid 已安装但未运行".to_string(),
                        description: "Waydroid (Android 兼容层) 已安装但容器未启动。".to_string(),
                        evidence: stdout.trim().to_string(),
                        fix: Some(FixAction {
                            description: "启动 Waydroid".to_string(),
                            command: "sudo waydroid container start && waydroid show-full-ui".to_string(),
                            risk_level: "low".to_string(),
                            ..Default::default()
                        }),
                        auto_fixable: false,
                    });
                }
            }
        }

        findings
    }

    /// 检查字体渲染配置
    fn check_font_rendering(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 检查 fontconfig 配置
        let _font_conf_paths = [
            "/etc/fonts/conf.d",
            "/etc/fonts/local.conf",
        ];

        // 检查中文字体优先级配置
        let mut has_chinese_priority = false;

        // 检查 64-language-selector-prefer.conf 或类似配置
        if let Ok(entries) = std::fs::read_dir("/etc/fonts/conf.d") {
            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains("chinese") || name.contains("cjk") || name.contains("zh") {
                    has_chinese_priority = true;
                    break;
                }
            }
        }

        // 检查用户级 fontconfig
        let home = std::env::var("HOME").unwrap_or_default();
        let user_fontconfig = format!("{}/.config/fontconfig/fonts.conf", home);
        if std::path::Path::new(&user_fontconfig).exists() {
            has_chinese_priority = true;
        }

        // 只有在有中文字体的情况下才检查渲染配置
        let has_chinese_fonts = command_output_with_timeout(
            Command::new("fc-list").args([":lang=zh"]),
            Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS),
        )
            .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(false);

        if has_chinese_fonts && !has_chinese_priority {
            findings.push(Finding {
                id: "sw-font-no-cjk-priority".to_string(),
                module: "software".to_string(),
                severity: Severity::Info,
                title: "未配置中文字体优先级".to_string(),
                description: "已安装中文字体但未配置 fontconfig 优先级，某些场景下中文可能使用非预期字体渲染。".to_string(),
                evidence: "no CJK font priority config found in /etc/fonts/conf.d/".to_string(),
                fix: Some(FixAction {
                    description: "配置中文字体优先级".to_string(),
                    command: r#"sudo bash -c 'cat > /etc/fonts/conf.d/64-chinese-prefer.conf << EOF
<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "fonts.dtd">
<fontconfig>
  <alias>
    <family>sans-serif</family>
    <prefer>
      <family>WenQuanYi Micro Hei</family>
      <family>Noto Sans CJK SC</family>
    </prefer>
  </alias>
  <alias>
    <family>serif</family>
    <prefer>
      <family>Noto Serif CJK SC</family>
    </prefer>
  </alias>
</fontconfig>
EOF'"#.to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: true,
            });
        }

        // 检查 hinting 和抗锯齿设置
        let fc_match_output = command_output_with_timeout(
            Command::new("fc-match").args(["--verbose", "sans-serif:lang=zh"]),
            Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS),
        );

        if let Some(o) = fc_match_output {
            let stdout = String::from_utf8_lossy(&o.stdout);

            // 检查是否启用了 hinting 和抗锯齿
            let has_hinting = stdout.contains("hinting: True") || stdout.contains("hintstyle:");
            let has_antialias = stdout.contains("antialias: True");

            if has_chinese_fonts && !has_hinting {
                findings.push(Finding {
                    id: "sw-font-no-hinting".to_string(),
                    module: "software".to_string(),
                    severity: Severity::Info,
                    title: "字体 hinting 未启用".to_string(),
                    description: "字体 hinting 未启用，中文显示可能不够清晰锐利。".to_string(),
                    evidence: format!("hinting={}", has_hinting),
                    fix: Some(FixAction {
                        description: "启用字体 hinting".to_string(),
                        command: "echo '在系统设置中启用字体 hinting，或执行: sudo dpkg-reconfigure fontconfig-config'".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            }

            if has_chinese_fonts && !has_antialias {
                findings.push(Finding {
                    id: "sw-font-no-antialias".to_string(),
                    module: "software".to_string(),
                    severity: Severity::Info,
                    title: "字体抗锯齿未启用".to_string(),
                    description: "字体抗锯齿未启用，文字边缘可能出现锯齿。".to_string(),
                    evidence: format!("antialias={}", has_antialias),
                    fix: Some(FixAction {
                        description: "启用字体抗锯齿".to_string(),
                        command: "echo '在系统设置中启用字体抗锯齿'".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            }

            // 如果没有中文字体匹配，可能是渲染问题
            if has_chinese_fonts && stdout.contains("NoMatch") {
                findings.push(Finding {
                    id: "sw-font-no-cjk-match".to_string(),
                    module: "software".to_string(),
                    severity: Severity::Warning,
                    title: "中文字体匹配失败".to_string(),
                    description: "fontconfig 无法为中文 (lang=zh) 匹配到合适的字体，中文显示可能异常。".to_string(),
                    evidence: stdout.lines().take(10).collect::<Vec<_>>().join("\n"),
                    fix: Some(FixAction {
                        description: "刷新字体缓存".to_string(),
                        command: "sudo fc-cache -fv".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        findings
    }

    /// 审计 Snap/Flatpak 已安装应用
    fn check_installed_apps(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        let timeout = Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS);

        // 审计 Snap 已安装应用
        if let Some(o) = command_output_with_timeout(
            Command::new("snap").args(["list"]),
            timeout,
        ) {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let _snap_count = stdout.lines().skip(1).filter(|l| !l.trim().is_empty()).count();

            // 检查是否有 classic confinement 的 snap（安全风险较高）
            let classic_snaps: Vec<&str> = stdout
                .lines()
                .skip(1)
                .filter(|l| l.contains("classic"))
                .collect();

            if !classic_snaps.is_empty() {
                findings.push(Finding {
                    id: "sw-snap-classic".to_string(),
                    module: "software".to_string(),
                    severity: Severity::Info,
                    title: format!("{} 个 Snap 应用使用 classic 模式", classic_snaps.len()),
                    description: format!(
                        "Classic confinement 的 Snap 应用可以访问宿主系统文件系统，安全隔离较弱。{}",
                        if classic_snaps.len() > 5 { "（仅显示前 5 个）" } else { "" }
                    ),
                    evidence: classic_snaps
                        .iter()
                        .take(5)
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                    fix: None,
                    auto_fixable: false,
                });
            }

            // 检查是否有 snap 应用需要刷新
            let refresh_output = command_output_with_timeout(
                Command::new("snap").args(["refresh", "--list"]),
                timeout,
            );

            if let Some(ro) = refresh_output {
                let rstdout = String::from_utf8_lossy(&ro.stdout);
                let refreshable = rstdout.lines().filter(|l| !l.trim().is_empty()).count();
                if refreshable > 0 {
                    findings.push(Finding {
                        id: "sw-snap-refresh".to_string(),
                        module: "software".to_string(),
                        severity: Severity::Info,
                        title: format!("{} 个 Snap 应用可更新", refreshable),
                        description: "部分 Snap 应用有可用更新，建议定期刷新以获取安全补丁。".to_string(),
                        evidence: rstdout.lines().take(10).collect::<Vec<_>>().join("\n"),
                        fix: Some(FixAction {
                            description: "更新所有 Snap 应用".to_string(),
                            command: "sudo snap refresh".to_string(),
                            risk_level: "low".to_string(),
                            ..Default::default()
                        }),
                        auto_fixable: true,
                    });
                }
            }
        }

        // 审计 Flatpak 已安装应用
        if let Some(o) = command_output_with_timeout(
            Command::new("flatpak").args(["list", "--app"]),
            timeout,
        ) {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let flatpak_count = stdout.lines().filter(|l| !l.trim().is_empty()).count();

            if flatpak_count > 0 {
                // 检查是否有未使用的 Flatpak 运行时（可以清理）
                let runtime_output = command_output_with_timeout(
                    Command::new("flatpak").args(["list", "--runtime", "--columns=application"]),
                    timeout,
                );

                if let Some(ro) = runtime_output {
                    let rstdout = String::from_utf8_lossy(&ro.stdout);
                    let runtime_count = rstdout.lines().filter(|l| !l.trim().is_empty()).count();

                    if runtime_count > 5 {
                        findings.push(Finding {
                            id: "sw-flatpak-runtimes".to_string(),
                            module: "software".to_string(),
                            severity: Severity::Info,
                            title: format!("安装了 {} 个 Flatpak 运行时", runtime_count),
                            description: "较多的 Flatpak 运行时可能占用大量磁盘空间，可清理未使用的运行时。".to_string(),
                            evidence: rstdout.lines().take(10).collect::<Vec<_>>().join("\n"),
                            fix: Some(FixAction {
                                description: "清理未使用的 Flatpak 运行时".to_string(),
                                command: "flatpak uninstall --unused".to_string(),
                                risk_level: "low".to_string(),
                                ..Default::default()
                            }),
                            auto_fixable: true,
                        });
                    }
                }
            }
        }

        findings
    }

    /// 检查 Snap/Flatpak 等通用包管理器
    fn check_universal_packages(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        let timeout = Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS);

        // 检查 Snap
        if check_command_version("snap", &["--version"]).is_some() {
            // 检查 snap 服务是否正常
            let output = command_output_with_timeout(
                Command::new("snap").args(["list"]),
                timeout,
            );

            if output.is_none() {
                findings.push(Finding {
                    id: "sw-snap-broken".to_string(),
                    module: "software".to_string(),
                    severity: Severity::Warning,
                    title: "Snap 包管理器异常".to_string(),
                    description: "Snap 已安装但无法正常工作，可能需要重新启动 snapd 服务。".to_string(),
                    evidence: "snap list failed".to_string(),
                    fix: Some(FixAction {
                        description: "重启 Snap 服务".to_string(),
                        command: "sudo systemctl restart snapd".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        // 检查 Flatpak
        if check_command_version("flatpak", &["--version"]).is_some() {
            let output = command_output_with_timeout(
                Command::new("flatpak").args(["remotes"]),
                timeout,
            );

            if let Some(o) = output {
                let stdout = String::from_utf8_lossy(&o.stdout);
                if stdout.trim().is_empty() {
                    findings.push(Finding {
                        id: "sw-flatpak-no-remotes".to_string(),
                        module: "software".to_string(),
                        severity: Severity::Info,
                        title: "Flatpak 未配置远程仓库".to_string(),
                        description: "Flatpak 已安装但没有配置远程仓库，无法安装 Flatpak 应用。".to_string(),
                        evidence: "flatpak remotes returned empty".to_string(),
                        fix: Some(FixAction {
                            description: "添加 Flathub 仓库".to_string(),
                            command: "sudo flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo".to_string(),
                            risk_level: "low".to_string(),
                            ..Default::default()
                        }),
                        auto_fixable: true,
                    });
                }
            }
        }

        findings
    }
}

impl Default for SoftwareDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for SoftwareDetector {
    fn name(&self) -> &str {
        "software"
    }

    fn description(&self) -> &str {
        "软件生态检测 — 包管理、依赖冲突、运行时环境、中文字体、兼容层、已安装应用"
    }

    fn scan(&self) -> anyhow::Result<ScanReport> {
        let start = Instant::now();
        let mut report = ScanReport::new("software".to_string());

        report.findings.extend(self.check_apt_status());
        report.findings.extend(self.check_apt_sources());
        report.findings.extend(self.check_dependency_conflicts());
        report.findings.extend(self.check_runtimes());
        report.findings.extend(self.check_chinese_fonts());
        report.findings.extend(self.check_font_rendering());
        report.findings.extend(self.check_wine());
        report.findings.extend(self.check_android_compat());
        report.findings.extend(self.check_universal_packages());
        report.findings.extend(self.check_installed_apps());

        report.duration_ms = start.elapsed().as_millis() as u64;
        Ok(report)
    }

    fn fix(&self, finding: &Finding) -> anyhow::Result<bool> {
        if let Some(ref fix_action) = finding.fix {
            fix_action.run_fix()
        } else {
            Ok(false)
        }
    }

    fn is_available(&self) -> bool {
        true
    }
}

/// 检查命令是否存在并获取版本
fn check_command_version(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .and_then(|o| {
            let output = if o.status.success() {
                String::from_utf8_lossy(&o.stdout).to_string()
            } else {
                String::from_utf8_lossy(&o.stderr).to_string()
            };
            if output.trim().is_empty() {
                None
            } else {
                Some(output.trim().to_string())
            }
        })
}
