use crate::detector::{Detector, Finding, FixAction, ScanReport, Severity};
use crate::util::{command_output_with_timeout, DEFAULT_CMD_TIMEOUT_SECS, LONG_CMD_TIMEOUT_SECS};
use std::process::Command;
use std::time::{Duration, Instant};

/// 安全审计检测模块
pub struct SecurityDetector;

impl SecurityDetector {
    pub fn new() -> Self {
        Self
    }

    /// 检查空密码账户
    fn check_empty_passwords(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let shadow = match std::fs::read_to_string("/etc/shadow") {
            Ok(s) => s,
            Err(_) => return findings, // 需要 root 权限
        };

        let mut empty_pass_users = Vec::new();

        for line in shadow.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 2 {
                continue;
            }
            let user = parts[0];
            let password_field = parts[1];

            // 空密码字段为 ""（两个连续冒号之间为空）
            if password_field.is_empty() {
                empty_pass_users.push(user.to_string());
            }
        }

        if !empty_pass_users.is_empty() {
            findings.push(Finding {
                id: "sec-empty-password".to_string(),
                module: "security".to_string(),
                severity: Severity::Critical,
                title: format!("{} 个账户存在空密码", empty_pass_users.len()),
                description: format!(
                    "以下账户没有设置密码，任何人可以直接登录：{}。这是严重的安全隐患。",
                    empty_pass_users.join(", ")
                ),
                evidence: format!("users={}", empty_pass_users.join(",")),
                fix: Some(FixAction {
                    description: "锁定空密码账户".to_string(),
                    command: empty_pass_users
                        .iter()
                        .map(|u| format!("sudo passwd -l {}", u))
                        .collect::<Vec<_>>()
                        .join(" && "),
                    risk_level: "low".to_string(),
                }),
                auto_fixable: true,
            });
        }

        findings
    }

    /// 检查 UID 为 0 的非 root 账户
    fn check_uid_zero(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let passwd = match std::fs::read_to_string("/etc/passwd") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        let mut uid_zero_users = Vec::new();

        for line in passwd.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 4 {
                continue;
            }
            let user = parts[0];
            let uid = parts[2];

            if uid == "0" && user != "root" {
                uid_zero_users.push(user.to_string());
            }
        }

        if !uid_zero_users.is_empty() {
            findings.push(Finding {
                id: "sec-uid-zero".to_string(),
                module: "security".to_string(),
                severity: Severity::Critical,
                title: format!("{} 个非 root 账户拥有 UID 0", uid_zero_users.len()),
                description: format!(
                    "以下账户拥有 root 权限（UID=0），可能是后门账户：{}。",
                    uid_zero_users.join(", ")
                ),
                evidence: format!("users={}", uid_zero_users.join(",")),
                fix: Some(FixAction {
                    description: "删除可疑的 UID 0 账户".to_string(),
                    command: uid_zero_users
                        .iter()
                        .map(|u| format!("sudo userdel {}", u))
                        .collect::<Vec<_>>()
                        .join(" && "),
                    risk_level: "high".to_string(),
                }),
                auto_fixable: false,
            });
        }

        findings
    }

    /// 检查 SUID/SGID 文件
    fn check_suid_files(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 检查常见系统目录中的 SUID 文件
        let output = match command_output_with_timeout(
            Command::new("find").args([
                "/usr/bin", "/usr/sbin", "/usr/local/bin", "/usr/local/sbin",
                "-perm", "-4000", "-type", "f",
            ]),
            Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS),
        ) {
            Some(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            None => return findings,
        };

        let suid_files: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();

        // 内置默认白名单
        let default_suid = [
            "/usr/bin/sudo", "/usr/bin/su", "/usr/bin/passwd", "/usr/bin/chsh",
            "/usr/bin/chfn", "/usr/bin/newgrp", "/usr/bin/gpasswd", "/usr/bin/mount",
            "/usr/bin/umount", "/usr/bin/fusermount", "/usr/bin/fusermount3",
            "/usr/bin/pkexec", "/usr/bin/crontab", "/usr/bin/at", "/usr/bin/ssh-agent",
            "/usr/bin/wall", "/usr/bin/write", "/usr/bin/bsd-write",
            "/usr/sbin/unix_chkpwd", "/usr/sbin/pam_extrausers_chkpwd",
        ];

        // 从用户配置文件加载额外白名单
        let home = std::env::var("HOME").unwrap_or_default();
        let whitelist_path = format!("{}/.kylin-doctor/suid_whitelist.txt", home);
        let user_whitelist: Vec<String> = std::fs::read_to_string(&whitelist_path)
            .ok()
            .map(|content| {
                content
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty() && !l.starts_with('#'))
                    .collect()
            })
            .unwrap_or_default();

        let unknown_suid: Vec<&str> = suid_files
            .iter()
            .filter(|f| {
                !default_suid.contains(f) && !user_whitelist.iter().any(|w| w.as_str() == **f)
            })
            .copied()
            .collect();

        if !unknown_suid.is_empty() {
            let whitelist_hint = if !std::path::Path::new(&whitelist_path).exists() {
                format!("\n\n提示: 可将误报路径添加到 {} 来排除", whitelist_path)
            } else {
                String::new()
            };

            findings.push(Finding {
                id: "sec-suid-unknown".to_string(),
                module: "security".to_string(),
                severity: Severity::Warning,
                title: format!("发现 {} 个非白名单 SUID 文件", unknown_suid.len()),
                description: format!(
                    "以下 SUID 文件不在已知白名单中，可能被恶意利用获取 root 权限。{}",
                    whitelist_hint
                ),
                evidence: unknown_suid.join("\n"),
                fix: Some(FixAction {
                    description: "审查并移除不必要的 SUID 位".to_string(),
                    command: format!(
                        "echo '请审查以下文件是否需要 SUID 位：\n{}'",
                        unknown_suid.join("\n")
                    ),
                    risk_level: "medium".to_string(),
                }),
                auto_fixable: false,
            });
        }

        findings
    }

    /// 检查关键目录权限
    fn check_directory_permissions(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let checks: Vec<(&str, u32, &str)> = vec![
            ("/etc", 0o755, "配置文件目录"),
            ("/etc/shadow", 0o640, "密码哈希文件"),
            ("/etc/passwd", 0o644, "用户信息文件"),
            ("/etc/ssh", 0o755, "SSH 配置目录"),
            ("/root", 0o700, "root 家目录"),
            ("/tmp", 0o1777, "临时文件目录"),
            ("/var/log", 0o755, "日志目录"),
        ];

        for (path, expected_mode, desc) in &checks {
            let metadata = match std::fs::metadata(path) {
                Ok(m) => m,
                Err(_) => continue,
            };

            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode() & 0o7777;

            if mode != *expected_mode {
                findings.push(Finding {
                    id: format!("sec-perm-{}", path.replace('/', "-").trim_start_matches('-')),
                    module: "security".to_string(),
                    severity: if *path == "/etc/shadow" || *path == "/root" {
                        Severity::Warning
                    } else {
                        Severity::Info
                    },
                    title: format!("{} ({}) 权限异常", path, desc),
                    description: format!(
                        "{} 当前权限 {:o}，期望 {:o}。",
                        path, mode, expected_mode
                    ),
                    evidence: format!("path={} mode={:o} expected={:o}", path, mode, expected_mode),
                    fix: Some(FixAction {
                        description: format!("修正 {} 权限", path),
                        command: format!("sudo chmod {:o} {}", expected_mode, path),
                        risk_level: "low".to_string(),
                    }),
                    auto_fixable: true,
                });
            }
        }

        findings
    }

    /// 检查 SSH 配置安全性
    fn check_ssh_config(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let sshd_config = match std::fs::read_to_string("/etc/ssh/sshd_config") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        // 也检查 sshd_config.d 目录
        let mut config_content = sshd_config.clone();
        if let Ok(entries) = std::fs::read_dir("/etc/ssh/sshd_config.d") {
            for entry in entries.filter_map(|e| e.ok()) {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    config_content.push('\n');
                    config_content.push_str(&content);
                }
            }
        }

        // 检查 root 登录
        let permit_root = get_ssh_setting(&config_content, "PermitRootLogin");
        if permit_root.as_deref() == Some("yes") {
            findings.push(Finding {
                id: "sec-ssh-root-login".to_string(),
                module: "security".to_string(),
                severity: Severity::Critical,
                title: "SSH 允许 root 直接登录".to_string(),
                description: "SSH 配置允许 root 用户直接登录，攻击者只需猜测密码即可获取最高权限。".to_string(),
                evidence: "PermitRootLogin yes".to_string(),
                fix: Some(FixAction {
                    description: "禁止 root SSH 登录".to_string(),
                    command: "sudo sed -i 's/^PermitRootLogin yes/PermitRootLogin no/' /etc/ssh/sshd_config && sudo systemctl restart sshd".to_string(),
                    risk_level: "medium".to_string(),
                }),
                auto_fixable: true,
            });
        }

        // 检查密码认证
        let password_auth = get_ssh_setting(&config_content, "PasswordAuthentication");
        if password_auth.as_deref() == Some("yes") {
            findings.push(Finding {
                id: "sec-ssh-password-auth".to_string(),
                module: "security".to_string(),
                severity: Severity::Warning,
                title: "SSH 启用密码认证".to_string(),
                description: "SSH 允许密码登录，容易受到暴力破解攻击。建议使用密钥认证。".to_string(),
                evidence: "PasswordAuthentication yes".to_string(),
                fix: Some(FixAction {
                    description: "改用密钥认证".to_string(),
                    command: "echo '建议先配置好 SSH 密钥，然后执行：sudo sed -i \"s/^PasswordAuthentication yes/PasswordAuthentication no/\" /etc/ssh/sshd_config && sudo systemctl restart sshd'".to_string(),
                    risk_level: "high".to_string(),
                }),
                auto_fixable: false,
            });
        }

        // 检查空密码登录
        let empty_pass = get_ssh_setting(&config_content, "PermitEmptyPasswords");
        if empty_pass.as_deref() == Some("yes") {
            findings.push(Finding {
                id: "sec-ssh-empty-password".to_string(),
                module: "security".to_string(),
                severity: Severity::Critical,
                title: "SSH 允许空密码登录".to_string(),
                description: "SSH 配置允许空密码登录，这是极其危险的配置。".to_string(),
                evidence: "PermitEmptyPasswords yes".to_string(),
                fix: Some(FixAction {
                    description: "禁止空密码登录".to_string(),
                    command: "sudo sed -i 's/^PermitEmptyPasswords yes/PermitEmptyPasswords no/' /etc/ssh/sshd_config && sudo systemctl restart sshd".to_string(),
                    risk_level: "low".to_string(),
                }),
                auto_fixable: true,
            });
        }

        // 检查 SSH 端口
        let port = get_ssh_setting(&config_content, "Port");
        if port.as_deref() == Some("22") || port.is_none() {
            findings.push(Finding {
                id: "sec-ssh-default-port".to_string(),
                module: "security".to_string(),
                severity: Severity::Info,
                title: "SSH 使用默认端口 22".to_string(),
                description: "SSH 使用默认端口 22，容易被自动化扫描工具发现。修改端口可减少攻击面。".to_string(),
                evidence: format!("Port {}", port.unwrap_or_else(|| "22 (default)".to_string())),
                fix: Some(FixAction {
                    description: "修改 SSH 端口".to_string(),
                    command: "echo '建议在 /etc/ssh/sshd_config 中修改 Port 为非标准端口（如 2222），并更新防火墙规则'".to_string(),
                    risk_level: "medium".to_string(),
                }),
                auto_fixable: false,
            });
        }

        findings
    }

    /// 检查防火墙状态
    fn check_firewall(&self) -> Vec<Finding> {
        let mut findings = Vec::new();
        let timeout = Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS);

        // 检查 ufw
        let ufw_output = command_output_with_timeout(
            Command::new("ufw").args(["status"]),
            timeout,
        );

        if let Some(o) = ufw_output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            if stdout.contains("Status: inactive") {
                findings.push(Finding {
                    id: "sec-fw-ufw-inactive".to_string(),
                    module: "security".to_string(),
                    severity: Severity::Warning,
                    title: "防火墙 (ufw) 未启用".to_string(),
                    description: "UFW 防火墙处于未激活状态，系统没有网络访问控制。".to_string(),
                    evidence: "Status: inactive".to_string(),
                    fix: Some(FixAction {
                        description: "启用防火墙".to_string(),
                        command: "sudo ufw enable".to_string(),
                        risk_level: "medium".to_string(),
                    }),
                    auto_fixable: false,
                });
            }
            return findings;
        }

        // 如果 ufw 不可用，检查 iptables
        let iptables_output = command_output_with_timeout(
            Command::new("iptables").args(["-L", "-n"]),
            timeout,
        );

        if let Some(o) = iptables_output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            // 如果所有链的策略都是 ACCEPT 且没有规则
            if stdout.contains("policy ACCEPT") && !stdout.contains("DROP") && !stdout.contains("REJECT") {
                findings.push(Finding {
                    id: "sec-fw-no-rules".to_string(),
                    module: "security".to_string(),
                    severity: Severity::Warning,
                    title: "iptables 无过滤规则".to_string(),
                    description: "iptables 防火墙策略为 ACCEPT 且无任何过滤规则，所有网络流量均被放行。".to_string(),
                    evidence: stdout.lines().take(10).collect::<Vec<_>>().join("\n"),
                    fix: Some(FixAction {
                        description: "配置防火墙规则".to_string(),
                        command: "echo '建议安装并启用 ufw：sudo apt-get install ufw && sudo ufw enable'".to_string(),
                        risk_level: "medium".to_string(),
                    }),
                    auto_fixable: false,
                });
            }
        }

        findings
    }

    /// 检查开放端口
    fn check_open_ports(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let output = match command_output_with_timeout(
            Command::new("ss").args(["-tlnp"]),
            Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS),
        ) {
            Some(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            None => return findings,
        };

        // 高风险端口列表
        let high_risk_ports: Vec<u16> = vec![
            23,    // telnet
            21,    // ftp
            69,    // tftp
            135,   // rpcbind
            139,   // netbios
            445,   // smb
            1433,  // mssql
            1521,  // oracle
            3306,  // mysql
            3389,  // rdp
            5432,  // postgresql
            5900,  // vnc
            6379,  // redis
            27017, // mongodb
        ];

        let mut exposed_high_risk = Vec::new();

        for line in output.lines().skip(1) {
            // 格式: LISTEN  0  128  0.0.0.0:22  0.0.0.0:*  users:(("sshd",pid=1234,fd=3))
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }

            let local_addr = parts[3];
            // 提取端口号
            if let Some(port_str) = local_addr.rsplit(':').next() {
                if let Ok(port) = port_str.parse::<u16>() {
                    // 检查是否绑定到所有接口（0.0.0.0 或 [::]）
                    let is_public = local_addr.starts_with("0.0.0.0:")
                        || local_addr.starts_with("[::]:")
                        || local_addr.starts_with("*:");

                    if is_public && high_risk_ports.contains(&port) {
                        exposed_high_risk.push((port, line.trim().to_string()));
                    }
                }
            }
        }

        if !exposed_high_risk.is_empty() {
            findings.push(Finding {
                id: "sec-ports-high-risk".to_string(),
                module: "security".to_string(),
                severity: Severity::Warning,
                title: format!("{} 个高风险端口对外暴露", exposed_high_risk.len()),
                description: format!(
                    "以下高风险端口绑定到所有网络接口，可能被外部攻击：{}",
                    exposed_high_risk
                        .iter()
                        .map(|(p, _)| p.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                evidence: exposed_high_risk
                    .iter()
                    .map(|(_, l)| l.clone())
                    .collect::<Vec<_>>()
                    .join("\n"),
                fix: Some(FixAction {
                    description: "限制高风险端口访问".to_string(),
                    command: "echo '建议通过防火墙限制这些端口的访问，或修改服务配置仅绑定到 127.0.0.1'".to_string(),
                    risk_level: "medium".to_string(),
                }),
                auto_fixable: false,
            });
        }

        findings
    }

    /// 检查最近的失败登录尝试
    fn check_failed_logins(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 尝试读取 auth.log 或 secure
        let log_paths = [
            "/var/log/auth.log",
            "/var/log/secure",
        ];

        for log_path in &log_paths {
            let content = match std::fs::read_to_string(log_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let failed_count = content
                .lines()
                .filter(|l| l.contains("Failed password") || l.contains("authentication failure"))
                .count();

            if failed_count > 20 {
                // 统计被攻击的用户名
                let mut user_counts: std::collections::HashMap<String, usize> =
                    std::collections::HashMap::new();
                for line in content.lines() {
                    if line.contains("Failed password") {
                        // 格式: Failed password for <user> from <ip>
                        if let Some(user_part) = line.split("Failed password for ").nth(1) {
                            let user = user_part.split_whitespace().next().unwrap_or("unknown");
                            *user_counts.entry(user.to_string()).or_insert(0) += 1;
                        }
                    }
                }

                let mut top_targets: Vec<(&String, &usize)> = user_counts.iter().collect();
                top_targets.sort_by(|a, b| b.1.cmp(a.1));
                let top_targets: Vec<String> = top_targets
                    .iter()
                    .take(5)
                    .map(|(u, c)| format!("{} ({}次)", u, c))
                    .collect();

                findings.push(Finding {
                    id: "sec-failed-logins".to_string(),
                    module: "security".to_string(),
                    severity: if failed_count > 100 {
                        Severity::Critical
                    } else {
                        Severity::Warning
                    },
                    title: format!("检测到 {} 次失败登录尝试", failed_count),
                    description: format!(
                        "日志中记录了大量失败登录，可能正在遭受暴力破解攻击。主要目标账户：{}",
                        top_targets.join(", ")
                    ),
                    evidence: format!(
                        "log={} failed_count={} top_targets={}",
                        log_path, failed_count, top_targets.join(",")
                    ),
                    fix: Some(FixAction {
                        description: "安装 fail2ban 防暴力破解".to_string(),
                        command: "sudo apt-get install -y fail2ban && sudo systemctl enable --now fail2ban".to_string(),
                        risk_level: "low".to_string(),
                    }),
                    auto_fixable: true,
                });
                break; // 只需要检查一个日志文件
            }
        }

        findings
    }

    /// 检查过期账户
    fn check_expired_accounts(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let shadow = match std::fs::read_to_string("/etc/shadow") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        // 获取当前日期（距离 1970-01-01 的天数）
        let today_days: i64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
            / 86400;

        let mut expired_users = Vec::new();
        let mut locked_inactive_users = Vec::new();

        for line in shadow.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() < 8 {
                continue;
            }
            let user = parts[0];
            let last_change = parts[2]; // 上次修改密码的日期（天数）
            let _max_days = parts[4];    // 密码最大有效期
            let _inactive = parts[6];    // 密码过期后的宽限期
            let expire = parts[7];      // 账户过期日期（天数）

            // 检查账户是否设置了过期日期
            if !expire.is_empty() && expire != "99999" {
                if let Ok(expire_days) = expire.parse::<i64>() {
                    if expire_days < today_days {
                        expired_users.push(format!("{} (过期于 {} 天前)", user, today_days - expire_days));
                    }
                }
            }

            // 检查密码是否长期未修改（超过 365 天）
            if !last_change.is_empty() && last_change != "0" {
                if let Ok(change_days) = last_change.parse::<i64>() {
                    let days_since_change = today_days - change_days;
                    if days_since_change > 365 {
                        locked_inactive_users.push(format!("{} ({} 天未改密)", user, days_since_change));
                    }
                }
            }
        }

        if !expired_users.is_empty() {
            findings.push(Finding {
                id: "sec-account-expired".to_string(),
                module: "security".to_string(),
                severity: Severity::Warning,
                title: format!("{} 个账户已过期", expired_users.len()),
                description: format!(
                    "以下账户已超过设定的过期日期，但仍可能被系统保留：{}",
                    if expired_users.len() > 5 {
                        format!("{}（仅显示前 5 个）", expired_users[..5].join(", "))
                    } else {
                        expired_users.join(", ")
                    }
                ),
                evidence: expired_users.join("\n"),
                fix: Some(FixAction {
                    description: "清理过期账户".to_string(),
                    command: "echo '使用 sudo userdel <用户名> 删除不需要的账户，或使用 sudo usermod -e YYYY-MM-DD <用户名> 更新过期日期'".to_string(),
                    risk_level: "low".to_string(),
                }),
                auto_fixable: false,
            });
        }

        if !locked_inactive_users.is_empty() {
            findings.push(Finding {
                id: "sec-account-stale-password".to_string(),
                module: "security".to_string(),
                severity: Severity::Info,
                title: format!("{} 个账户密码超过 365 天未修改", locked_inactive_users.len()),
                description: format!(
                    "以下账户密码长期未修改，可能存在安全风险：{}",
                    if locked_inactive_users.len() > 5 {
                        format!("{}（仅显示前 5 个）", locked_inactive_users[..5].join(", "))
                    } else {
                        locked_inactive_users.join(", ")
                    }
                ),
                evidence: locked_inactive_users.join("\n"),
                fix: Some(FixAction {
                    description: "要求修改密码".to_string(),
                    command: "echo '使用 sudo chage -M 90 <用户名> 设置密码有效期为 90 天'".to_string(),
                    risk_level: "low".to_string(),
                }),
                auto_fixable: false,
            });
        }

        findings
    }

    /// 检查审计日志（auditd + 系统日志）
    fn check_audit_logs(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 检查 auditd 是否运行
        let auditd_running = command_output_with_timeout(
            Command::new("systemctl").args(["is-active", "auditd"]),
            Duration::from_secs(DEFAULT_CMD_TIMEOUT_SECS),
        )
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
            .unwrap_or(false);

        if !auditd_running {
            // auditd 未运行，只做提示
            findings.push(Finding {
                id: "sec-audit-not-running".to_string(),
                module: "security".to_string(),
                severity: Severity::Info,
                title: "审计守护进程 (auditd) 未运行".to_string(),
                description: "auditd 未运行，无法记录系统审计事件。建议启用以满足安全合规要求。".to_string(),
                evidence: "systemctl is-active auditd => not active".to_string(),
                fix: Some(FixAction {
                    description: "启用 auditd".to_string(),
                    command: "sudo apt-get install -y auditd && sudo systemctl enable --now auditd".to_string(),
                    risk_level: "low".to_string(),
                }),
                auto_fixable: true,
            });
        }

        // 检查可疑的 sudo 使用
        let log_paths = ["/var/log/auth.log", "/var/log/secure"];
        for log_path in &log_paths {
            let content = match std::fs::read_to_string(log_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // 统计 sudo 使用情况
            let sudo_events: Vec<&str> = content
                .lines()
                .filter(|l| l.contains("sudo:") && l.contains("COMMAND="))
                .collect();

            // 检查是否有非预期用户使用 sudo
            let mut sudo_users: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for event in &sudo_events {
                // 格式: ... sudo: user : TTY=... ; PWD=... ; USER=root ; COMMAND=...
                if let Some(user_part) = event.split("sudo:").nth(1) {
                    if let Some(user) = user_part.split(':').next() {
                        let user = user.trim();
                        if !user.is_empty() {
                            *sudo_users.entry(user.to_string()).or_insert(0) += 1;
                        }
                    }
                }
            }

            // 检查高频率 sudo 使用（可能是异常）
            let high_freq: Vec<String> = sudo_users
                .iter()
                .filter(|(_, &count)| count > 100)
                .map(|(u, c)| format!("{} ({}次)", u, c))
                .collect();

            if !high_freq.is_empty() {
                findings.push(Finding {
                    id: "sec-audit-sudo-high-freq".to_string(),
                    module: "security".to_string(),
                    severity: Severity::Info,
                    title: "部分用户 sudo 使用频率较高".to_string(),
                    description: format!(
                        "以下用户在审计日志中 sudo 使用超过 100 次：{}。高频 sudo 可能是正常运维，也可能是异常。",
                        high_freq.join(", ")
                    ),
                    evidence: high_freq.join("\n"),
                    fix: None,
                    auto_fixable: false,
                });
            }

            // 检查 sudo 失败（权限不足尝试）
            let sudo_failures = content
                .lines()
                .filter(|l| l.contains("sudo:") && l.contains("authentication failure"))
                .count();

            if sudo_failures > 10 {
                findings.push(Finding {
                    id: "sec-audit-sudo-failures".to_string(),
                    module: "security".to_string(),
                    severity: Severity::Warning,
                    title: format!("检测到 {} 次 sudo 认证失败", sudo_failures),
                    description: "大量 sudo 认证失败可能表明有人在尝试提权。".to_string(),
                    evidence: format!("sudo_auth_failures={}", sudo_failures),
                    fix: Some(FixAction {
                        description: "检查 sudo 日志详情".to_string(),
                        command: format!("grep 'sudo.*authentication failure' {} | tail -20", log_path),
                        risk_level: "low".to_string(),
                    }),
                    auto_fixable: false,
                });
            }

            break; // 只检查一个日志文件
        }

        // 检查 /var/log/ 目录权限
        let log_dir = "/var/log";
        if let Ok(metadata) = std::fs::metadata(log_dir) {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode() & 0o7777;
            // /var/log 不应该是全局可写的
            if mode & 0o002 != 0 {
                findings.push(Finding {
                    id: "sec-audit-log-dir-world-writable".to_string(),
                    module: "security".to_string(),
                    severity: Severity::Warning,
                    title: "/var/log 目录全局可写".to_string(),
                    description: format!(
                        "/var/log 目录权限为 {:o}，包含全局可写位。攻击者可以篡改日志文件掩盖入侵痕迹。",
                        mode
                    ),
                    evidence: format!("mode={:o}", mode),
                    fix: Some(FixAction {
                        description: "修复 /var/log 权限".to_string(),
                        command: "sudo chmod 755 /var/log".to_string(),
                        risk_level: "low".to_string(),
                    }),
                    auto_fixable: true,
                });
            }
        }

        findings
    }

    /// 简化版 CVE/漏洞检查（基于内核版本和已知问题）
    fn check_known_vulnerabilities(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 读取内核版本
        let kernel_version = match std::fs::read_to_string("/proc/version") {
            Ok(v) => v,
            Err(_) => return findings,
        };

        let kernel_ver = kernel_version
            .split_whitespace()
            .nth(2)
            .unwrap_or("unknown");

        // 检查内核是否过旧（简单判断主版本号）
        // 格式: 5.4.0-xx-generic 或 5.15.0-xx-generic
        if let Some(major_minor) = kernel_ver.split('.').take(2).collect::<Vec<&str>>().first().map(|s| *s) {
            if let Ok(major) = major_minor.parse::<u32>() {
                if major < 5 {
                    findings.push(Finding {
                        id: "sec-vuln-kernel-old".to_string(),
                        module: "security".to_string(),
                        severity: Severity::Warning,
                        title: format!("内核版本过旧 ({})", kernel_ver),
                        description: format!(
                            "当前内核 {} 版本较旧，可能存在已知安全漏洞。建议升级到最新 LTS 内核。",
                            kernel_ver
                        ),
                        evidence: format!("kernel={}", kernel_ver),
                        fix: Some(FixAction {
                            description: "检查内核更新".to_string(),
                            command: "sudo apt-get update && apt list --upgradable 2>/dev/null | grep linux-image".to_string(),
                            risk_level: "medium".to_string(),
                        }),
                        auto_fixable: false,
                    });
                }
            }
        }

        // 检查是否有待安装的安全更新（apt 可能因锁阻塞，用较长超时）
        let security_output = command_output_with_timeout(
            Command::new("apt-get").args(["-s", "upgrade"]),
            Duration::from_secs(LONG_CMD_TIMEOUT_SECS),
        );

        if let Some(o) = security_output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let security_updates = stdout
                .lines()
                .filter(|l| l.contains("security"))
                .count();

            if security_updates > 0 {
                findings.push(Finding {
                    id: "sec-vuln-security-updates".to_string(),
                    module: "security".to_string(),
                    severity: Severity::Warning,
                    title: format!("{} 个安全更新待安装", security_updates),
                    description: format!(
                        "有 {} 个安全更新尚未安装，这些更新修复了已知漏洞。建议尽快安装。",
                        security_updates
                    ),
                    evidence: format!("security_updates_pending={}", security_updates),
                    fix: Some(FixAction {
                        description: "安装安全更新".to_string(),
                        command: "sudo apt-get update && sudo apt-get upgrade -y".to_string(),
                        risk_level: "low".to_string(),
                    }),
                    auto_fixable: true,
                });
            }
        }

        // 检查 unattended-upgrades 是否配置
        let unattended_config = "/etc/apt/apt.conf.d/20auto-upgrades";
        if !std::path::Path::new(unattended_config).exists() {
            findings.push(Finding {
                id: "sec-vuln-no-auto-upgrades".to_string(),
                module: "security".to_string(),
                severity: Severity::Info,
                title: "未配置自动安全更新".to_string(),
                description: "unattended-upgrades 未配置，系统不会自动安装安全更新。建议启用自动安全更新。".to_string(),
                evidence: format!("{} not found", unattended_config),
                fix: Some(FixAction {
                    description: "启用自动安全更新".to_string(),
                    command: "sudo apt-get install -y unattended-upgrades && sudo dpkg-reconfigure -plow unattended-upgrades".to_string(),
                    risk_level: "low".to_string(),
                }),
                auto_fixable: true,
            });
        }

        // 检查是否启用了 ASLR
        let aslr = std::fs::read_to_string("/proc/sys/kernel/randomize_va_space")
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);

        if aslr != 2 {
            findings.push(Finding {
                id: "sec-vuln-no-aslr".to_string(),
                module: "security".to_string(),
                severity: Severity::Warning,
                title: format!("ASLR 未完全启用 (值={})", aslr),
                description: "地址空间布局随机化 (ASLR) 未设置为完全随机化 (2)，降低了对抗内存攻击的能力。".to_string(),
                evidence: format!("randomize_va_space={}", aslr),
                fix: Some(FixAction {
                    description: "启用完整 ASLR".to_string(),
                    command: "sudo sysctl -w kernel.randomize_va_space=2 && echo 'kernel.randomize_va_space=2' | sudo tee -a /etc/sysctl.conf".to_string(),
                    risk_level: "low".to_string(),
                }),
                auto_fixable: true,
            });
        }

        // 检查是否启用了核心转储限制
        let core_pattern = std::fs::read_to_string("/proc/sys/kernel/core_pattern")
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        // 如果 core_pattern 是空的且没有 ulimit 限制，核心转储可能泄露敏感信息
        let core_ulimit = Command::new("sh")
            .args(["-c", "ulimit -c"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "0".to_string());

        if core_ulimit == "unlimited" && (core_pattern.is_empty() || core_pattern.starts_with("core")) {
            findings.push(Finding {
                id: "sec-vuln-core-dump".to_string(),
                module: "security".to_string(),
                severity: Severity::Info,
                title: "核心转储未受限".to_string(),
                description: "核心转储未限制大小且使用默认模式，崩溃进程的内存（可能含密码、密钥）会被写入磁盘。".to_string(),
                evidence: format!("core_pattern={} ulimit -c={}", core_pattern, core_ulimit),
                fix: Some(FixAction {
                    description: "限制核心转储".to_string(),
                    command: "echo '* hard core 0' | sudo tee -a /etc/security/limits.conf".to_string(),
                    risk_level: "low".to_string(),
                }),
                auto_fixable: true,
            });
        }

        findings
    }

    /// 检查密码策略
    fn check_password_policy(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 检查 /etc/login.defs 中的密码策略
        let login_defs = match std::fs::read_to_string("/etc/login.defs") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        let mut max_days = 99999;
        let mut _min_days = 0;
        let mut min_len = 5;

        for line in login_defs.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            match parts[0] {
                "PASS_MAX_DAYS" => {
                    max_days = parts[1].parse().unwrap_or(99999);
                }
                "PASS_MIN_DAYS" => {
                    _min_days = parts[1].parse().unwrap_or(0);
                }
                "PASS_MIN_LEN" => {
                    min_len = parts[1].parse().unwrap_or(5);
                }
                _ => {}
            }
        }

        if max_days > 365 {
            findings.push(Finding {
                id: "sec-pass-max-days".to_string(),
                module: "security".to_string(),
                severity: Severity::Info,
                title: format!("密码最大有效期过长 ({} 天)", max_days),
                description: format!(
                    "密码最大有效期设置为 {} 天，建议不超过 365 天以降低密码泄露风险。",
                    max_days
                ),
                evidence: format!("PASS_MAX_DAYS={}", max_days),
                fix: Some(FixAction {
                    description: "缩短密码有效期".to_string(),
                    command: "sudo sed -i 's/^PASS_MAX_DAYS.*/PASS_MAX_DAYS 90/' /etc/login.defs".to_string(),
                    risk_level: "low".to_string(),
                }),
                auto_fixable: true,
            });
        }

        if min_len < 8 {
            findings.push(Finding {
                id: "sec-pass-min-len".to_string(),
                module: "security".to_string(),
                severity: Severity::Info,
                title: format!("密码最小长度过短 ({} 位)", min_len),
                description: format!(
                    "密码最小长度设置为 {} 位，建议至少 8 位以提高密码强度。",
                    min_len
                ),
                evidence: format!("PASS_MIN_LEN={}", min_len),
                fix: Some(FixAction {
                    description: "增加密码最小长度".to_string(),
                    command: "sudo sed -i 's/^PASS_MIN_LEN.*/PASS_MIN_LEN 8/' /etc/login.defs".to_string(),
                    risk_level: "low".to_string(),
                }),
                auto_fixable: true,
            });
        }

        findings
    }
}

impl Default for SecurityDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for SecurityDetector {
    fn name(&self) -> &str {
        "security"
    }

    fn description(&self) -> &str {
        "安全审计检测 — 用户账户、过期账户、文件权限、SSH、防火墙、开放端口、审计日志、漏洞检查"
    }

    fn scan(&self) -> anyhow::Result<ScanReport> {
        let start = Instant::now();
        let mut report = ScanReport::new("security".to_string());

        report.findings.extend(self.check_empty_passwords());
        report.findings.extend(self.check_uid_zero());
        report.findings.extend(self.check_expired_accounts());
        report.findings.extend(self.check_suid_files());
        report.findings.extend(self.check_directory_permissions());
        report.findings.extend(self.check_ssh_config());
        report.findings.extend(self.check_firewall());
        report.findings.extend(self.check_open_ports());
        report.findings.extend(self.check_failed_logins());
        report.findings.extend(self.check_audit_logs());
        report.findings.extend(self.check_known_vulnerabilities());
        report.findings.extend(self.check_password_policy());

        report.duration_ms = start.elapsed().as_millis() as u64;
        Ok(report)
    }

    fn fix(&self, finding: &Finding) -> anyhow::Result<bool> {
        if let Some(ref fix_action) = finding.fix {
            let status = Command::new("sh")
                .args(["-c", &fix_action.command])
                .status()?;
            Ok(status.success())
        } else {
            Ok(false)
        }
    }

    fn is_available(&self) -> bool {
        true
    }
}

/// 从 sshd_config 中提取配置项值
fn get_ssh_setting(config: &str, key: &str) -> Option<String> {
    for line in config.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(2, char::is_whitespace).collect();
        if parts.len() == 2 && parts[0].eq_ignore_ascii_case(key) {
            return Some(parts[1].trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_ssh_setting_basic() {
        let config = "PermitRootLogin no\nPasswordAuthentication yes\n#Port 22\n";
        assert_eq!(get_ssh_setting(config, "PermitRootLogin"), Some("no".to_string()));
        assert_eq!(get_ssh_setting(config, "PasswordAuthentication"), Some("yes".to_string()));
    }

    #[test]
    fn get_ssh_setting_case_insensitive() {
        let config = "permitrootlogin no\n";
        assert_eq!(get_ssh_setting(config, "PermitRootLogin"), Some("no".to_string()));
    }

    #[test]
    fn get_ssh_setting_skip_comments() {
        let config = "#PermitRootLogin yes\nPermitRootLogin no\n";
        assert_eq!(get_ssh_setting(config, "PermitRootLogin"), Some("no".to_string()));
    }

    #[test]
    fn get_ssh_setting_not_found() {
        let config = "PermitRootLogin no\n";
        assert_eq!(get_ssh_setting(config, "NonExistent"), None);
    }

    #[test]
    fn get_ssh_setting_empty_config() {
        assert_eq!(get_ssh_setting("", "PermitRootLogin"), None);
    }
}
