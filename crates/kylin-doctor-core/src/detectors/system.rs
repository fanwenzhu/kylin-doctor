use crate::detector::{Detector, Finding, FixAction, ScanReport, Severity};
use std::process::Command;
use std::time::Instant;

/// 系统健康检测模块
pub struct SystemDetector;

impl SystemDetector {
    pub fn new() -> Self {
        Self
    }

    /// 检查磁盘空间使用率
    fn check_disk_space(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let output = match Command::new("df").args(["-h", "--output=target,pcent"]).output() {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return findings,
        };

        for line in output.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let mount = parts[0];
            let pct_str = parts[1].trim_end_matches('%');
            let pct: u32 = match pct_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };

            if pct >= 90 {
                findings.push(Finding {
                    id: format!("system-disk-critical-{}", mount.replace('/', "_")),
                    module: "system".to_string(),
                    severity: Severity::Critical,
                    title: format!("{} 磁盘空间严重不足 ({}%)", mount, pct),
                    description: format!(
                        "挂载点 {} 使用率已达 {}%，可能导致系统不稳定或服务异常。",
                        mount, pct
                    ),
                    evidence: line.to_string(),
                    fix: Some(FixAction {
                        description: "清理临时文件和旧日志".to_string(),
                        command: "sudo journalctl --vacuum-time=7d && sudo apt-get clean".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            } else if pct >= 80 {
                findings.push(Finding {
                    id: format!("system-disk-warning-{}", mount.replace('/', "_")),
                    module: "system".to_string(),
                    severity: Severity::Warning,
                    title: format!("{} 磁盘空间偏高 ({}%)", mount, pct),
                    description: format!(
                        "挂载点 {} 使用率已达 {}%，建议清理不必要的文件。",
                        mount, pct
                    ),
                    evidence: line.to_string(),
                    fix: Some(FixAction {
                        description: "清理临时文件".to_string(),
                        command: "sudo apt-get clean && rm -rf /tmp/*".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        findings
    }

    /// 检查 systemd 服务状态
    fn check_services(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let output = match Command::new("systemctl")
            .args(["list-units", "--type=service", "--state=failed", "--no-pager", "--no-legend"])
            .output()
        {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return findings,
        };

        for line in output.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // 格式: ● UNIT LOAD ACTIVE SUB DESCRIPTION (● 是 systemd 状态指示符)
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 5 {
                continue;
            }
            // 跳过开头的 ● 符号
            let unit = parts[1];

            findings.push(Finding {
                id: format!("system-svc-failed-{}", unit.replace('.', "-")),
                module: "system".to_string(),
                severity: Severity::Warning,
                title: format!("服务 {} 启动失败", unit),
                description: format!("systemd 服务 {} 当前处于 failed 状态，需要排查原因。", unit),
                evidence: line.to_string(),
                fix: Some(FixAction {
                    description: format!("重启服务 {}", unit),
                    command: format!("sudo systemctl restart {}", unit),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: true,
            });
        }

        findings
    }

    /// 检查僵尸进程
    fn check_zombie_processes(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let output = match Command::new("ps")
            .args(["-eo", "pid,ppid,stat,comm"])
            .output()
        {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return findings,
        };

        let mut zombie_count = 0;
        let mut zombie_details = Vec::new();

        for line in output.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }
            let stat = parts[2];
            if stat.contains('Z') {
                zombie_count += 1;
                zombie_details.push(format!("  PID={} PPID={} CMD={}", parts[0], parts[1], parts[3]));
            }
        }

        if zombie_count > 0 {
            findings.push(Finding {
                id: "system-zombie-processes".to_string(),
                module: "system".to_string(),
                severity: if zombie_count > 5 {
                    Severity::Warning
                } else {
                    Severity::Info
                },
                title: format!("发现 {} 个僵尸进程", zombie_count),
                description: "僵尸进程未被父进程回收，通常是程序 bug 导致。少量僵尸不影响系统，但大量僵尸可能耗尽 PID 资源。".to_string(),
                evidence: zombie_details.join("\n"),
                fix: None,
                auto_fixable: false,
            });
        }

        findings
    }

    /// 检查 dmesg 中的异常
    fn check_dmesg(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let output = match Command::new("dmesg").args(["--level=err,crit,alert,emerg", "-T"]).output() {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => {
                // dmesg 可能需要 root 权限，尝试不带 -T
                match Command::new("dmesg").args(["--level=err,crit,alert,emerg"]).output() {
                    Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
                    Err(_) => return findings,
                }
            }
        };

        if !output.trim().is_empty() {
            let lines: Vec<&str> = output.lines().collect();
            let recent_errors: Vec<&str> = lines.iter().rev().take(5).cloned().collect();

            findings.push(Finding {
                id: "system-dmesg-errors".to_string(),
                module: "system".to_string(),
                severity: Severity::Warning,
                title: format!("内核日志中有 {} 条错误记录", lines.len()),
                description: "dmesg 中存在错误级别日志，可能指示硬件故障或驱动问题。".to_string(),
                evidence: recent_errors.join("\n"),
                fix: None,
                auto_fixable: false,
            });
        }

        findings
    }

    /// 检查系统负载
    fn check_load_average(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let output = match Command::new("cat").args(["/proc/loadavg"]).output() {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return findings,
        };

        let parts: Vec<&str> = output.split_whitespace().collect();
        if parts.len() < 3 {
            return findings;
        }

        let load_1m: f64 = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => return findings,
        };

        // 获取 CPU 核心数
        let nproc_output = Command::new("nproc").output();
        let nproc: f64 = nproc_output
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().unwrap_or(1.0))
            .unwrap_or(1.0);

        let load_ratio = load_1m / nproc;

        if load_ratio > 2.0 {
            findings.push(Finding {
                id: "system-load-critical".to_string(),
                module: "system".to_string(),
                severity: Severity::Critical,
                title: format!("系统负载极高 ({:.2} / {}核)", load_1m, nproc as u32),
                description: format!(
                    "1分钟负载 {:.2}，CPU 核心数 {}，负载比 {:.1}x，系统可能已卡顿。",
                    load_1m, nproc as u32, load_ratio
                ),
                evidence: output.trim().to_string(),
                fix: None,
                auto_fixable: false,
            });
        } else if load_ratio > 1.0 {
            findings.push(Finding {
                id: "system-load-warning".to_string(),
                module: "system".to_string(),
                severity: Severity::Warning,
                title: format!("系统负载偏高 ({:.2} / {}核)", load_1m, nproc as u32),
                description: format!(
                    "1分钟负载 {:.2}，CPU 核心数 {}，负载比 {:.1}x，建议关注高耗进程。",
                    load_1m, nproc as u32, load_ratio
                ),
                evidence: output.trim().to_string(),
                fix: None,
                auto_fixable: false,
            });
        }

        findings
    }
}

impl Default for SystemDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for SystemDetector {
    fn name(&self) -> &str {
        "system"
    }

    fn description(&self) -> &str {
        "系统健康检测 — 磁盘空间、服务状态、僵尸进程、内核日志、系统负载"
    }

    fn scan(&self) -> anyhow::Result<ScanReport> {
        let start = Instant::now();
        let mut report = ScanReport::new("system".to_string());

        report.findings.extend(self.check_disk_space());
        report.findings.extend(self.check_services());
        report.findings.extend(self.check_zombie_processes());
        report.findings.extend(self.check_dmesg());
        report.findings.extend(self.check_load_average());

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
