use crate::detector::{Detector, Finding, FixAction, ScanReport, Severity};
use crate::util::{parse_diskstats, parse_meminfo, read_sysfs_u64};
use std::process::Command;
use std::time::Instant;

/// 性能分析检测模块
pub struct PerformanceDetector;

impl PerformanceDetector {
    pub fn new() -> Self {
        Self
    }

    /// 检查 CPU 使用率（采样 1 秒）
    fn check_cpu_usage(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 读取 /proc/stat 两次，间隔 1 秒，计算使用率
        let stat1 = match std::fs::read_to_string("/proc/stat") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        std::thread::sleep(std::time::Duration::from_secs(1));

        let stat2 = match std::fs::read_to_string("/proc/stat") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        let cpu1 = parse_cpu_stat(&stat1);
        let cpu2 = parse_cpu_stat(&stat2);

        if let (Some((idle1, total1)), Some((idle2, total2))) = (cpu1, cpu2) {
            let total_diff = total2 - total1;
            let idle_diff = idle2 - idle1;

            if total_diff == 0 {
                return findings;
            }

            let usage_pct = ((total_diff - idle_diff) as f64 / total_diff as f64) * 100.0;

            if usage_pct > 95.0 {
                findings.push(Finding {
                    id: "perf-cpu-critical".to_string(),
                    module: "performance".to_string(),
                    severity: Severity::Critical,
                    title: format!("CPU 使用率极高 ({:.0}%)", usage_pct),
                    description: format!(
                        "CPU 使用率 {:.0}%，系统可能已严重卡顿。建议检查高耗进程。",
                        usage_pct
                    ),
                    evidence: format!("usage={:.1}% sample_interval=1s", usage_pct),
                    fix: Some(FixAction {
                        description: "查看高耗进程".to_string(),
                        command: "top -bn1 -o %CPU | head -20".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            } else if usage_pct > 80.0 {
                findings.push(Finding {
                    id: "perf-cpu-warning".to_string(),
                    module: "performance".to_string(),
                    severity: Severity::Warning,
                    title: format!("CPU 使用率偏高 ({:.0}%)", usage_pct),
                    description: format!(
                        "CPU 使用率 {:.0}%，建议关注是否有异常进程占用过多 CPU。",
                        usage_pct
                    ),
                    evidence: format!("usage={:.1}% sample_interval=1s", usage_pct),
                    fix: Some(FixAction {
                        description: "查看高耗进程".to_string(),
                        command: "top -bn1 -o %CPU | head -20".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            }
        }

        findings
    }

    /// 检查系统负载趋势
    fn check_load_trend(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let loadavg = match std::fs::read_to_string("/proc/loadavg") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        let parts: Vec<&str> = loadavg.split_whitespace().collect();
        if parts.len() < 3 {
            return findings;
        }

        let load_1m: f64 = parts[0].parse().unwrap_or(0.0);
        let load_5m: f64 = parts[1].parse().unwrap_or(0.0);
        let load_15m: f64 = parts[2].parse().unwrap_or(0.0);

        let nproc = Command::new("nproc")
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<f64>().ok())
            .unwrap_or(1.0);

        // 负载持续上升趋势
        if load_1m > load_5m && load_5m > load_15m && load_1m / nproc > 1.5 {
            findings.push(Finding {
                id: "perf-load-rising".to_string(),
                module: "performance".to_string(),
                severity: Severity::Warning,
                title: "系统负载持续上升".to_string(),
                description: format!(
                    "1分钟负载 {:.2} > 5分钟 {:.2} > 15分钟 {:.2}（{}核），负载呈上升趋势，系统压力在增大。",
                    load_1m, load_5m, load_15m, nproc as u32
                ),
                evidence: format!(
                    "1m={:.2} 5m={:.2} 15m={:.2} nproc={}",
                    load_1m, load_5m, load_15m, nproc as u32
                ),
                fix: Some(FixAction {
                    description: "查看高耗进程".to_string(),
                    command: "top -bn1 -o %CPU | head -20".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: false,
            });
        }

        // 负载持续下降但仍然很高
        if load_1m < load_5m && load_5m < load_15m && load_15m / nproc > 2.0 {
            findings.push(Finding {
                id: "perf-load-recovering".to_string(),
                module: "performance".to_string(),
                severity: Severity::Info,
                title: "系统负载正在恢复中".to_string(),
                description: format!(
                    "1分钟负载 {:.2} < 5分钟 {:.2} < 15分钟 {:.2}（{}核），之前有过高负载，目前正在恢复。",
                    load_1m, load_5m, load_15m, nproc as u32
                ),
                evidence: format!(
                    "1m={:.2} 5m={:.2} 15m={:.2} nproc={}",
                    load_1m, load_5m, load_15m, nproc as u32
                ),
                fix: None,
                auto_fixable: false,
            });
        }

        findings
    }

    /// 检查内存性能指标
    fn check_memory_performance(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let meminfo = match std::fs::read_to_string("/proc/meminfo") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        let values = parse_meminfo(&meminfo);

        let mem_total = values.get("MemTotal").copied().unwrap_or(0);
        let _mem_available = values.get("MemAvailable").copied().unwrap_or(0);
        let swap_total = values.get("SwapTotal").copied().unwrap_or(0);
        let swap_free = values.get("SwapFree").copied().unwrap_or(0);
        let dirty = values.get("Dirty").copied().unwrap_or(0);
        let writeback = values.get("Writeback").copied().unwrap_or(0);

        if mem_total == 0 {
            return findings;
        }

        // 检查 Swap 使用情况
        if swap_total > 0 {
            let swap_used = swap_total - swap_free;
            let swap_pct = (swap_used as f64 / swap_total as f64) * 100.0;

            if swap_pct > 80.0 {
                findings.push(Finding {
                    id: "perf-swap-high".to_string(),
                    module: "performance".to_string(),
                    severity: Severity::Warning,
                    title: format!("Swap 使用率过高 ({:.0}%)", swap_pct),
                    description: format!(
                        "Swap 已使用 {:.0}%（{} MB / {} MB），大量使用 Swap 会严重降低系统性能。",
                        swap_pct,
                        swap_used / 1024,
                        swap_total / 1024
                    ),
                    evidence: format!(
                        "SwapTotal={}kB SwapFree={}kB usage={:.1}%",
                        swap_total, swap_free, swap_pct
                    ),
                    fix: Some(FixAction {
                        description: "清理内存缓存或关闭高耗进程".to_string(),
                        command: "sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        // 检查脏页数量（大量待写入数据）
        let dirty_mb = dirty / 1024;
        if dirty_mb > 500 {
            findings.push(Finding {
                id: "perf-dirty-pages".to_string(),
                module: "performance".to_string(),
                severity: Severity::Warning,
                title: format!("大量脏页待写入 ({} MB)", dirty_mb),
                description: format!(
                    "有 {} MB 脏页等待写入磁盘，可能导致系统卡顿或数据丢失风险。",
                    dirty_mb
                ),
                evidence: format!("Dirty={}kB Writeback={}kB", dirty, writeback),
                fix: Some(FixAction {
                    description: "强制刷写脏页到磁盘".to_string(),
                    command: "sudo sync".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: true,
            });
        }

        // 检查透明大页状态
        let thp = std::fs::read_to_string("/sys/kernel/mm/transparent_hugepage/enabled")
            .ok()
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if thp.contains("[always]") {
            findings.push(Finding {
                id: "perf-thp-always".to_string(),
                module: "performance".to_string(),
                severity: Severity::Info,
                title: "透明大页设置为 always".to_string(),
                description: "透明大页 (THP) 设置为 always，在某些场景下（如数据库）可能导致延迟抖动。".to_string(),
                evidence: format!("transparent_hugepage={}", thp),
                fix: Some(FixAction {
                    description: "将 THP 设为 madvise 模式".to_string(),
                    command: "sudo sh -c 'echo madvise > /sys/kernel/mm/transparent_hugepage/enabled'".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: true,
            });
        }

        findings
    }

    /// 检查磁盘 I/O 性能
    fn check_disk_io(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 读取 /proc/diskstats 两次，间隔 1 秒
        let stats1 = match std::fs::read_to_string("/proc/diskstats") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        std::thread::sleep(std::time::Duration::from_secs(1));

        let stats2 = match std::fs::read_to_string("/proc/diskstats") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        let disk1 = parse_diskstats(&stats1);
        let disk2 = parse_diskstats(&stats2);

        for (device, s2) in &disk2 {
            if device.starts_with("loop") || device.starts_with("ram") {
                continue;
            }

            let s1 = match disk1.get(device) {
                Some(s) => s,
                None => continue,
            };

            let reads_diff = s2.reads_completed.saturating_sub(s1.reads_completed);
            let writes_diff = s2.writes_completed.saturating_sub(s1.writes_completed);
            let io_time_diff = s2.io_time_ms.saturating_sub(s1.io_time_ms);
            let _read_time_diff = s2.read_time_ms.saturating_sub(s1.read_time_ms);
            let _write_time_diff = s2.write_time_ms.saturating_sub(s1.write_time_ms);

            let total_io = reads_diff + writes_diff;
            if total_io == 0 {
                continue;
            }

            // 计算平均 I/O 延迟
            let avg_latency_ms = io_time_diff as f64 / total_io as f64;

            // 计算 I/O 利用率（io_time / 1000ms * 100%）
            let io_util_pct = (io_time_diff as f64 / 1000.0) * 100.0;

            if avg_latency_ms > 100.0 {
                findings.push(Finding {
                    id: format!("perf-disk-io-critical-{}", device),
                    module: "performance".to_string(),
                    severity: Severity::Critical,
                    title: format!("磁盘 {} I/O 延迟极高 ({:.0}ms)", device, avg_latency_ms),
                    description: format!(
                        "设备 {} 平均 I/O 延迟 {:.1}ms，I/O 利用率 {:.0}%，磁盘可能已成为性能瓶颈。",
                        device, avg_latency_ms, io_util_pct
                    ),
                    evidence: format!(
                        "device={} avg_latency={:.1}ms io_util={:.0}% reads/s={} writes/s={}",
                        device, avg_latency_ms, io_util_pct, reads_diff, writes_diff
                    ),
                    fix: Some(FixAction {
                        description: "检查磁盘健康和 I/O 进程".to_string(),
                        command: format!("iotop -o -P -d 1 -n 3 && smartctl -H /dev/{}", device),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            } else if avg_latency_ms > 20.0 || io_util_pct > 80.0 {
                findings.push(Finding {
                    id: format!("perf-disk-io-warning-{}", device),
                    module: "performance".to_string(),
                    severity: Severity::Warning,
                    title: format!("磁盘 {} I/O 压力偏高 (延迟 {:.0}ms, 利用率 {:.0}%)", device, avg_latency_ms, io_util_pct),
                    description: format!(
                        "设备 {} 平均延迟 {:.1}ms，I/O 利用率 {:.0}%，可能存在 I/O 瓶颈。",
                        device, avg_latency_ms, io_util_pct
                    ),
                    evidence: format!(
                        "device={} avg_latency={:.1}ms io_util={:.0}% reads/s={} writes/s={}",
                        device, avg_latency_ms, io_util_pct, reads_diff, writes_diff
                    ),
                    fix: Some(FixAction {
                        description: "查看 I/O 占用进程".to_string(),
                        command: "iotop -o -P -d 1 -n 3".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            }
        }

        findings
    }

    /// 检查网络连接数
    fn check_network_connections(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let output = match Command::new("ss")
            .args(["-s"])
            .output()
        {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return findings,
        };

        // 解析 TCP 连接数
        let mut tcp_established = 0;
        let mut tcp_timewait = 0;
        let mut tcp_total = 0;

        for line in output.lines() {
            if line.starts_with("TCP:") {
                // 格式: TCP:   123 (estab 80, closed 30, orphaned 0, timewait 10)
                let parts: Vec<&str> = line.split(&['(', ',', ')'][..]).collect();
                for part in &parts {
                    let part = part.trim();
                    if part.starts_with("estab") {
                        tcp_established = part
                            .split_whitespace()
                            .nth(1)
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                    } else if part.starts_with("timewait") {
                        tcp_timewait = part
                            .split_whitespace()
                            .nth(1)
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                    }
                }
                // TCP: 后面的第一个数字是总数
                if let Some(total_str) = line.split_whitespace().nth(1) {
                    tcp_total = total_str.parse().unwrap_or(0);
                }
            }
        }

        if tcp_timewait > 5000 {
            findings.push(Finding {
                id: "perf-net-timewait".to_string(),
                module: "performance".to_string(),
                severity: Severity::Warning,
                title: format!("TCP TIME_WAIT 连接过多 ({})", tcp_timewait),
                description: format!(
                    "当前有 {} 个 TIME_WAIT 连接，可能消耗大量端口资源，影响新连接建立。",
                    tcp_timewait
                ),
                evidence: format!(
                    "tcp_established={} tcp_timewait={} tcp_total={}",
                    tcp_established, tcp_timewait, tcp_total
                ),
                fix: Some(FixAction {
                    description: "优化 TCP 参数".to_string(),
                    command: "sudo sysctl -w net.ipv4.tcp_tw_reuse=1".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: true,
            });
        }

        if tcp_established > 5000 {
            findings.push(Finding {
                id: "perf-net-connections-high".to_string(),
                module: "performance".to_string(),
                severity: Severity::Warning,
                title: format!("TCP 已建立连接数过多 ({})", tcp_established),
                description: format!(
                    "当前有 {} 个已建立的 TCP 连接，可能是正常的高并发，也可能是连接泄漏。",
                    tcp_established
                ),
                evidence: format!(
                    "tcp_established={} tcp_timewait={} tcp_total={}",
                    tcp_established, tcp_timewait, tcp_total
                ),
                fix: Some(FixAction {
                    description: "查看连接详情".to_string(),
                    command: "ss -tnp | awk '{print $5}' | cut -d: -f1 | sort | uniq -c | sort -rn | head -20".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: false,
            });
        }

        findings
    }

    /// 检查网络延迟
    fn check_network_latency(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // ping 网关
        let gateway = get_default_gateway();
        if let Some(ref gw) = gateway {
            let output = Command::new("ping")
                .args(["-c", "3", "-W", "2", gw])
                .output();

            if let Ok(o) = output {
                let stdout = String::from_utf8_lossy(&o.stdout);

                // 解析 avg 延迟
                // 格式: rtt min/avg/max/mdev = 0.123/0.456/0.789/0.012 ms
                if let Some(rtt_line) = stdout.lines().find(|l| l.contains("rtt") || l.contains("round-trip")) {
                    if let Some(eq_part) = rtt_line.split('=').nth(1) {
                        let vals: Vec<&str> = eq_part.trim().split('/').collect();
                        if vals.len() >= 3 {
                            if let Ok(avg_ms) = vals[1].trim().parse::<f64>() {
                                if avg_ms > 50.0 {
                                    findings.push(Finding {
                                        id: "perf-net-latency-high".to_string(),
                                        module: "performance".to_string(),
                                        severity: Severity::Warning,
                                        title: format!("网络延迟偏高 ({:.1}ms 到网关)", avg_ms),
                                        description: format!(
                                            "到默认网关 {} 的平均延迟 {:.1}ms，可能存在网络问题。",
                                            gw, avg_ms
                                        ),
                                        evidence: format!("gateway={} avg_ms={:.1}", gw, avg_ms),
                                        fix: Some(FixAction {
                                            description: "检查网络连接".to_string(),
                                            command: format!("ping -c 10 {} && ip route show", gw),
                                            risk_level: "low".to_string(),
                                            ..Default::default()
                                        }),
                                        auto_fixable: false,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        findings
    }

    /// 检查电源/电池状态
    fn check_power(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 检查是否有电池
        let bat_path = std::path::Path::new("/sys/class/power_supply/BAT0");
        if !bat_path.exists() {
            return findings;
        }

        // 读取电池容量
        let capacity = std::fs::read_to_string(bat_path.join("capacity"))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok());

        // 读取电池状态
        let status = std::fs::read_to_string(bat_path.join("status"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        // 读取电池健康
        let health = std::fs::read_to_string(bat_path.join("health"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        if let Some(cap) = capacity {
            if status == "Discharging" && cap < 10 {
                findings.push(Finding {
                    id: "perf-battery-low".to_string(),
                    module: "performance".to_string(),
                    severity: Severity::Warning,
                    title: format!("电池电量极低 ({}%)", cap),
                    description: format!(
                        "电池电量仅 {}%，状态为 {}，建议立即连接电源。",
                        cap, status
                    ),
                    evidence: format!("capacity={} status={}", cap, status),
                    fix: Some(FixAction {
                        description: "连接电源适配器".to_string(),
                        command: "echo '请连接电源适配器为电池充电'".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            }
        }

        if health == "Degraded" || health == "Dead" {
            findings.push(Finding {
                id: "perf-battery-health".to_string(),
                module: "performance".to_string(),
                severity: if health == "Dead" {
                    Severity::Critical
                } else {
                    Severity::Warning
                },
                title: format!("电池健康状态: {}", health),
                description: format!(
                    "电池健康状态为 {}，续航能力已下降，建议更换电池。",
                    health
                ),
                evidence: format!("health={}", health),
                fix: Some(FixAction {
                    description: "检查电池详情".to_string(),
                    command: "upower -i /org/freedesktop/UPower/devices/battery_BAT0".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: false,
            });
        }

        // 检查 CPU 调频策略
        let governor_path = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor";
        if let Ok(governor) = std::fs::read_to_string(governor_path) {
            let governor = governor.trim();
            if governor == "performance" {
                findings.push(Finding {
                    id: "perf-cpu-governor-performance".to_string(),
                    module: "performance".to_string(),
                    severity: Severity::Info,
                    title: "CPU 调频策略为 performance".to_string(),
                    description: "CPU 调频策略设置为 performance（最高频率），会增加功耗和发热。笔记本用户可考虑 powersave 模式。".to_string(),
                    evidence: format!("governor={}", governor),
                    fix: Some(FixAction {
                        description: "切换为按需调频".to_string(),
                        command: "sudo cpupower frequency-set -g ondemand".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        findings
    }

    /// 检查桌面合成器帧率
    fn check_desktop_compositor(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 检测运行中的合成器
        let compositors = [
            ("kwin_x11", "KWin (KDE)"),
            ("kwin_wayland", "KWin Wayland (KDE)"),
            ("mutter", "Mutter (GNOME)"),
            ("xfwm4", "Xfwm4 (XFCE)"),
            ("marco", "Marco (MATE)"),
            ("compton", "Compton"),
            ("picom", "Picom"),
        ];

        let mut running_compositor = None;
        for (proc_name, display_name) in &compositors {
            let is_running = Command::new("pgrep")
                .args(["-x", proc_name])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);

            if is_running {
                running_compositor = Some((*proc_name, *display_name));
                break;
            }
        }

        if let Some((proc_name, display_name)) = running_compositor {
            // 获取合成器进程的 CPU 和内存使用
            let ps_output = Command::new("ps")
                .args(["-C", proc_name, "-o", "%cpu,%mem,rss,etime", "--no-headers"])
                .output();

            if let Ok(o) = ps_output {
                let stdout = String::from_utf8_lossy(&o.stdout);
                if let Some(line) = stdout.lines().next() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 3 {
                        let cpu_pct: f64 = parts[0].parse().unwrap_or(0.0);
                        let mem_pct: f64 = parts[1].parse().unwrap_or(0.0);
                        let rss_kb: u64 = parts[2].parse().unwrap_or(0);

                        // 合成器 CPU 占用过高
                        if cpu_pct > 30.0 {
                            findings.push(Finding {
                                id: "perf-compositor-cpu-high".to_string(),
                                module: "performance".to_string(),
                                severity: Severity::Warning,
                                title: format!("桌面合成器 {} CPU 占用过高 ({:.0}%)", display_name, cpu_pct),
                                description: format!(
                                    "合成器进程 {} CPU 使用率 {:.0}%，可能导致桌面卡顿。可能是特效过多或显卡驱动问题。",
                                    display_name, cpu_pct
                                ),
                                evidence: format!(
                                    "compositor={} cpu={:.1}% mem={:.1}% rss={}kB",
                                    proc_name, cpu_pct, mem_pct, rss_kb
                                ),
                                fix: Some(FixAction {
                                    description: "检查合成器设置".to_string(),
                                    command: format!("echo '建议关闭桌面特效或检查显卡驱动：\n  KDE: 系统设置 → 显示 → 合成器\n  GNOME: 优化工具 → 外观 → 动画'"),
                                    risk_level: "low".to_string(),
                                    ..Default::default()
                                }),
                                auto_fixable: false,
                            });
                        }

                        // 合成器内存占用过高（超过 500MB）
                        let rss_mb = rss_kb / 1024;
                        if rss_mb > 500 {
                            findings.push(Finding {
                                id: "perf-compositor-mem-high".to_string(),
                                module: "performance".to_string(),
                                severity: Severity::Info,
                                title: format!("桌面合成器 {} 内存占用较高 ({} MB)", display_name, rss_mb),
                                description: format!(
                                    "合成器进程 {} 占用 {} MB 内存，长时间运行可能存在内存泄漏。",
                                    display_name, rss_mb
                                ),
                                evidence: format!(
                                    "compositor={} cpu={:.1}% mem={:.1}% rss={}MB",
                                    proc_name, cpu_pct, mem_pct, rss_mb
                                ),
                                fix: Some(FixAction {
                                    description: "重启合成器".to_string(),
                                    command: format!("echo '可尝试重启桌面合成器释放内存'"),
                                    risk_level: "low".to_string(),
                                    ..Default::default()
                                }),
                                auto_fixable: false,
                            });
                        }
                    }
                }
            }

            // 检查 X11 帧率（如果有 xrandr）
            if proc_name.contains("x11") || proc_name == "mutter" || proc_name == "xfwm4" {
                let xrandr_output = Command::new("xrandr")
                    .args(["--query"])
                    .output();

                if let Ok(o) = xrandr_output {
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    for line in stdout.lines() {
                        // 格式: eDP-1 connected primary 1920x1080+0+0 ... 60.00*+ 59.93
                        if line.contains("connected") && line.contains('*') {
                            // 提取当前刷新率
                            if let Some(rate_part) = line.split_whitespace().find(|s| s.ends_with("*+") || s.ends_with("*")) {
                                let rate_str = rate_part.trim_end_matches('*').trim_end_matches('+');
                                if let Ok(rate) = rate_str.parse::<f64>() {
                                    if rate < 30.0 {
                                        findings.push(Finding {
                                            id: "perf-compositor-low-refresh".to_string(),
                                            module: "performance".to_string(),
                                            severity: Severity::Info,
                                            title: format!("显示器刷新率较低 ({:.0} Hz)", rate),
                                            description: format!(
                                                "当前显示器刷新率 {:.0} Hz，较低的刷新率可能导致画面不流畅。",
                                                rate
                                            ),
                                            evidence: format!("refresh_rate={:.0}Hz display={}", rate, line.split_whitespace().next().unwrap_or("?")),
                                            fix: Some(FixAction {
                                                description: "检查显示器设置".to_string(),
                                                command: "xrandr --query".to_string(),
                                                risk_level: "low".to_string(),
                                                ..Default::default()
                                            }),
                                            auto_fixable: false,
                                        });
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        findings
    }

    /// 检查内存碎片率
    fn check_memory_fragmentation(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 读取 /proc/buddyinfo 分析内存碎片
        let buddyinfo = match std::fs::read_to_string("/proc/buddyinfo") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        // /proc/buddyinfo 格式:
        // Node 0, zone   Normal   1    2    3    4    5    6    7    8    9   10
        // 每列代表 2^order 个连续页面的空闲块数量
        // order 0 = 4KB, order 1 = 8KB, ..., order 10 = 4MB
        // 如果高阶 (order >= 8, 即 1MB+) 空闲块很少，说明碎片严重

        for line in buddyinfo.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 12 {
                continue;
            }

            let zone = parts[3]; // Normal, DMA, etc.

            // 解析各 order 的空闲块数
            let counts: Vec<u64> = parts[4..]
                .iter()
                .filter_map(|s| s.parse().ok())
                .collect();

            if counts.len() < 11 {
                continue;
            }

            // 计算碎片化指标：高阶(order>=8)空闲块占总空闲块的比例
            let total_free: u64 = counts.iter().sum();
            let high_order_free: u64 = counts[8..].iter().sum();

            if total_free == 0 {
                continue;
            }

            let high_order_pct = (high_order_free as f64 / total_free as f64) * 100.0;

            // 如果高阶空闲块占比低于 5%，碎片化严重
            if high_order_pct < 5.0 && total_free > 100 {
                findings.push(Finding {
                    id: format!("perf-mem-frag-{}", zone),
                    module: "performance".to_string(),
                    severity: Severity::Warning,
                    title: format!("内存碎片化严重 (zone: {}, 高阶块占比 {:.1}%)", zone, high_order_pct),
                    description: format!(
                        "zone {} 高阶 (>=1MB) 空闲内存块仅占 {:.1}%，碎片化可能导致大块内存分配失败或性能下降。",
                        zone, high_order_pct
                    ),
                    evidence: format!(
                        "zone={} total_free={} high_order_free={} high_order_pct={:.1}% counts={:?}",
                        zone, total_free, high_order_free, high_order_pct, counts
                    ),
                    fix: Some(FixAction {
                        description: "触发内存整理".to_string(),
                        command: "sudo sh -c 'echo 1 > /proc/sys/vm/compact_memory'".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        // 读取 /proc/pagetypeinfo 补充检查
        let vmstat = std::fs::read_to_string("/proc/vmstat").unwrap_or_default();
        let mut compact_fail = 0u64;
        let mut compact_stall = 0u64;
        for line in vmstat.lines() {
            if line.starts_with("compact_fail") {
                compact_fail = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            } else if line.starts_with("compact_stall") {
                compact_stall = line.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
            }
        }

        if compact_stall > 100 {
            findings.push(Finding {
                id: "perf-mem-compact-stall".to_string(),
                module: "performance".to_string(),
                severity: Severity::Warning,
                title: format!("内存压缩频繁阻塞 ({} 次)", compact_stall),
                description: format!(
                    "系统累计发生 {} 次内存压缩阻塞 (compact_stall)，{} 次压缩失败。内存碎片化已影响系统性能。",
                    compact_stall, compact_fail
                ),
                evidence: format!("compact_stall={} compact_fail={}", compact_stall, compact_fail),
                fix: Some(FixAction {
                    description: "检查内存使用情况".to_string(),
                    command: "cat /proc/buddyinfo && echo '---' && cat /proc/meminfo | grep -E 'MemTotal|MemFree|MemAvailable'".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: false,
            });
        }

        findings
    }

    /// 检查磁盘 IOPS 和队列深度
    fn check_disk_iops(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 读取 /proc/diskstats 两次，间隔 1 秒
        let stats1 = match std::fs::read_to_string("/proc/diskstats") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        std::thread::sleep(std::time::Duration::from_secs(1));

        let stats2 = match std::fs::read_to_string("/proc/diskstats") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        let disk1 = parse_diskstats(&stats1);
        let disk2 = parse_diskstats(&stats2);

        for (device, d2) in &disk2 {
            if device.starts_with("loop") || device.starts_with("ram") || device.starts_with("dm-") {
                continue;
            }

            let d1 = match disk1.get(device) {
                Some(d) => d,
                None => continue,
            };

            let reads_per_sec = d2.reads_completed.saturating_sub(d1.reads_completed);
            let writes_per_sec = d2.writes_completed.saturating_sub(d1.writes_completed);
            let total_iops = reads_per_sec + writes_per_sec;
            let queue_depth = d2.io_in_progress; // 当前队列深度

            // 高 IOPS 报告
            if total_iops > 0 || queue_depth > 0 {
                let severity = if queue_depth > 32 {
                    Severity::Warning
                } else {
                    Severity::Info
                };

                // 只在有明显 I/O 或队列深度较高时报告
                if total_iops > 100 || queue_depth > 8 {
                    findings.push(Finding {
                        id: format!("perf-disk-iops-{}", device),
                        module: "performance".to_string(),
                        severity,
                        title: format!("磁盘 {} IOPS: {} (队列深度: {})", device, total_iops, queue_depth),
                        description: format!(
                            "设备 {} 每秒 I/O 操作: 读 {} + 写 {} = {}，当前队列深度 {}。{}",
                            device, reads_per_sec, writes_per_sec, total_iops, queue_depth,
                            if queue_depth > 32 { "队列过深，I/O 可能堆积。" } else { "" }
                        ),
                        evidence: format!(
                            "device={} read_iops={} write_iops={} total_iops={} queue_depth={}",
                            device, reads_per_sec, writes_per_sec, total_iops, queue_depth
                        ),
                        fix: if queue_depth > 32 {
                            Some(FixAction {
                                description: "检查 I/O 密集型进程".to_string(),
                                command: "iotop -o -P -d 1 -n 3".to_string(),
                                risk_level: "low".to_string(),
                                ..Default::default()
                            })
                        } else {
                            None
                        },
                        auto_fixable: false,
                    });
                }
            }
        }

        findings
    }

    /// 检查网络带宽和吞吐量
    fn check_network_bandwidth(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 从 /sys/class/net/*/speed 获取链路速度
        if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
            for entry in entries.filter_map(|e| e.ok()) {
                let iface = entry.file_name().to_string_lossy().to_string();
                if iface == "lo" {
                    continue;
                }

                let iface_path = entry.path();
                let is_physical = iface_path.join("device").exists();
                if !is_physical {
                    continue;
                }

                // 读取链路速度 (Mbps)
                let speed = std::fs::read_to_string(iface_path.join("speed"))
                    .ok()
                    .and_then(|s| s.trim().parse::<i32>().ok());

                if let Some(speed_mbps) = speed {
                    if speed_mbps > 0 {
                        // 读取错误率来判断链路质量
                        let rx_errors = read_sysfs_u64(iface_path.join("statistics/rx_errors"));
                        let rx_packets = read_sysfs_u64(iface_path.join("statistics/rx_packets"));
                        let tx_errors = read_sysfs_u64(iface_path.join("statistics/tx_errors"));
                        let tx_packets = read_sysfs_u64(iface_path.join("statistics/tx_packets"));

                        let total_packets = rx_packets + tx_packets;
                        let total_errors = rx_errors + tx_errors;

                        // 计算丢包率
                        if total_packets > 1000 {
                            let error_rate = (total_errors as f64 / total_packets as f64) * 100.0;
                            if error_rate > 0.1 {
                                findings.push(Finding {
                                    id: format!("perf-net-error-rate-{}", iface),
                                    module: "performance".to_string(),
                                    severity: Severity::Warning,
                                    title: format!("网卡 {} 丢包率偏高 ({:.2}%)", iface, error_rate),
                                    description: format!(
                                        "网卡 {} 链路速度 {}Mbps，但丢包率 {:.2}%（{} 错误 / {} 包），可能影响网络性能。",
                                        iface, speed_mbps, error_rate, total_errors, total_packets
                                    ),
                                    evidence: format!(
                                        "iface={} speed={}Mbps rx_errors={} tx_errors={} rx_packets={} tx_packets={} error_rate={:.4}%",
                                        iface, speed_mbps, rx_errors, tx_errors, rx_packets, tx_packets, error_rate
                                    ),
                                    fix: Some(FixAction {
                                        description: "检查网线和交换机端口".to_string(),
                                        command: format!("ethtool {} && ethtool -S {}", iface, iface),
                                        risk_level: "low".to_string(),
                                        ..Default::default()
                                    }),
                                    auto_fixable: false,
                                });
                            }
                        }

                        // 低速链路提示
                        if speed_mbps < 100 && speed_mbps > 0 {
                            findings.push(Finding {
                                id: format!("perf-net-low-speed-{}", iface),
                                module: "performance".to_string(),
                                severity: Severity::Info,
                                title: format!("网卡 {} 链路速度较低 ({}Mbps)", iface, speed_mbps),
                                description: format!(
                                    "网卡 {} 当前链路速度仅 {}Mbps，可能限制网络传输性能。",
                                    iface, speed_mbps
                                ),
                                evidence: format!("iface={} speed={}Mbps", iface, speed_mbps),
                                fix: Some(FixAction {
                                    description: "检查网卡和网线".to_string(),
                                    command: format!("ethtool {}", iface),
                                    risk_level: "low".to_string(),
                                    ..Default::default()
                                }),
                                auto_fixable: false,
                            });
                        }
                    }
                }

                // 读取 /proc/net/dev 计算吞吐量
                let net_dev = std::fs::read_to_string("/proc/net/dev").unwrap_or_default();
                for dev_line in net_dev.lines() {
                    if !dev_line.contains(&iface) {
                        continue;
                    }
                    let parts: Vec<&str> = dev_line.split_whitespace().collect();
                    // 格式: iface: rx_bytes rx_packets rx_errs ... tx_bytes tx_packets tx_errs ...
                    if parts.len() >= 10 {
                        let rx_bytes: u64 = parts[1].parse().unwrap_or(0);
                        let tx_bytes: u64 = parts[9].parse().unwrap_or(0);
                        let total_gb = (rx_bytes + tx_bytes) as f64 / (1024.0 * 1024.0 * 1024.0);

                        if total_gb > 100.0 {
                            findings.push(Finding {
                                id: format!("perf-net-throughput-{}", iface),
                                module: "performance".to_string(),
                                severity: Severity::Info,
                                title: format!("网卡 {} 累计吞吐量 {:.1} GB", iface, total_gb),
                                description: format!(
                                    "网卡 {} 自启动以来累计传输 {:.1} GB 数据（接收 {:.1} GB，发送 {:.1} GB）。",
                                    iface, total_gb, rx_bytes as f64 / (1024.0 * 1024.0 * 1024.0), tx_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
                                ),
                                evidence: format!(
                                    "iface={} rx_bytes={} tx_bytes={} total={:.1}GB",
                                    iface, rx_bytes, tx_bytes, total_gb
                                ),
                                fix: None,
                                auto_fixable: false,
                            });
                        }
                    }
                    break;
                }
            }
        }

        findings
    }

    /// 检查 CPU 调度延迟
    fn check_cpu_scheduling(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 读取 /proc/schedstat（如果可用）
        let schedstat = match std::fs::read_to_string("/proc/schedstat") {
            Ok(s) => s,
            Err(_) => {
                // schedstat 不可用，尝试其他方式
                return self.check_cpu_scheduling_fallback();
            }
        };

        // /proc/schedstat 格式:
        // cpu<N> <yield_count> <schedule_count> <sched_count> <sched_goidle> <ttwu_count> <ttwu_local> <rq_cpu_time> <rq_sched_info.run_delay> <rq_sched_info.pcount>
        // 或者版本 15+:
        // cpu<N> <timestamp> <timestamp>

        for line in schedstat.lines() {
            if !line.starts_with("cpu") {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 10 {
                continue;
            }

            let cpu_id = parts[0];
            let run_delay: u64 = parts[8].parse().unwrap_or(0); // 总调度延迟（纳秒）
            let pcount: u64 = parts[9].parse().unwrap_or(0);    // 调度次数

            if pcount > 0 {
                let avg_delay_us = (run_delay / pcount) / 1000; // 平均延迟（微秒）

                // 平均调度延迟超过 10ms 表示有调度问题
                if avg_delay_us > 10000 {
                    findings.push(Finding {
                        id: format!("perf-cpu-sched-delay-{}", cpu_id),
                        module: "performance".to_string(),
                        severity: Severity::Warning,
                        title: format!("{} 调度延迟偏高 ({}μs)", cpu_id, avg_delay_us),
                        description: format!(
                            "{} 平均调度延迟 {}μs（{}ms），进程等待 CPU 时间过长，可能导致响应延迟。",
                            cpu_id, avg_delay_us, avg_delay_us / 1000
                        ),
                        evidence: format!(
                            "{} run_delay={}ns pcount={} avg_delay={}μs",
                            cpu_id, run_delay, pcount, avg_delay_us
                        ),
                        fix: Some(FixAction {
                            description: "检查 CPU 密集型进程".to_string(),
                            command: "top -bn1 -o %CPU | head -20".to_string(),
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

    /// CPU 调度延迟的后备检查方式
    fn check_cpu_scheduling_fallback(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 通过 /proc/stat 检查 iowait 比例
        let stat = match std::fs::read_to_string("/proc/stat") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        for line in stat.lines() {
            if !line.starts_with("cpu") {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 8 {
                continue;
            }

            // 解析 iowait
            let iowait: u64 = parts[5].parse().unwrap_or(0);
            let total: u64 = parts[1..]
                .iter()
                .filter_map(|v| v.parse::<u64>().ok())
                .sum();

            if total > 0 {
                let iowait_pct = (iowait as f64 / total as f64) * 100.0;
                let cpu_id = parts[0];

                if iowait_pct > 20.0 && cpu_id == "cpu" {
                    findings.push(Finding {
                        id: "perf-cpu-iowait-high".to_string(),
                        module: "performance".to_string(),
                        severity: Severity::Warning,
                        title: format!("CPU I/O 等待偏高 ({:.0}%)", iowait_pct),
                        description: format!(
                            "CPU I/O 等待时间占 {:.0}%，进程频繁等待磁盘 I/O 完成，系统响应可能变慢。",
                            iowait_pct
                        ),
                        evidence: format!("iowait={:.1}% total_ticks={}", iowait_pct, total),
                        fix: Some(FixAction {
                            description: "检查磁盘 I/O 瓶颈".to_string(),
                            command: "iotop -o -P -d 1 -n 3".to_string(),
                            risk_level: "low".to_string(),
                            ..Default::default()
                        }),
                        auto_fixable: false,
                    });
                }
            }
            break; // 只看总体 cpu 行
        }

        findings
    }

    /// 检查 I/O 调度器
    fn check_io_scheduler(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 遍历所有块设备
        let block_dir = match std::fs::read_dir("/sys/block") {
            Ok(d) => d,
            Err(_) => return findings,
        };

        for entry in block_dir.filter_map(|e| e.ok()) {
            let device = entry.file_name().to_string_lossy().to_string();

            // 跳过 loop、ram 等虚拟设备
            if device.starts_with("loop") || device.starts_with("ram") || device.starts_with("dm-") {
                continue;
            }

            let scheduler_path = entry.path().join("queue/scheduler");
            let scheduler = match std::fs::read_to_string(&scheduler_path) {
                Ok(s) => s.trim().to_string(),
                Err(_) => continue,
            };

            // 解析当前调度器（带 [] 的是当前使用的）
            let current = scheduler
                .split_whitespace()
                .find(|s| s.starts_with('['))
                .map(|s| s.trim_matches(|c| c == '[' || c == ']'))
                .unwrap_or("");

            // 对于 SSD，建议使用 noop/none/mq-deadline
            let rotational_path = entry.path().join("queue/rotational");
            let is_rotational = std::fs::read_to_string(rotational_path)
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
                .unwrap_or(1);

            if is_rotational == 0 && current == "cfq" {
                findings.push(Finding {
                    id: format!("perf-io-sched-ssd-{}", device),
                    module: "performance".to_string(),
                    severity: Severity::Info,
                    title: format!("SSD {} 使用 cfq 调度器", device),
                    description: format!(
                        "SSD 设备 {} 当前使用 cfq 调度器，建议使用 mq-deadline 或 none 以获得更好性能。",
                        device
                    ),
                    evidence: format!("device={} scheduler={}", device, scheduler),
                    fix: Some(FixAction {
                        description: "切换 I/O 调度器".to_string(),
                        command: format!(
                            "echo mq-deadline | sudo tee /sys/block/{}/queue/scheduler",
                            device
                        ),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        findings
    }
}

impl Default for PerformanceDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for PerformanceDetector {
    fn name(&self) -> &str {
        "performance"
    }

    fn description(&self) -> &str {
        "性能分析检测 — CPU/调度延迟、内存/碎片、磁盘 I/O/IOPS、网络延迟/带宽、桌面合成器、电源"
    }

    fn scan(&self) -> anyhow::Result<ScanReport> {
        let start = Instant::now();
        let mut report = ScanReport::new("performance".to_string());

        report.findings.extend(self.check_cpu_usage());
        report.findings.extend(self.check_cpu_scheduling());
        report.findings.extend(self.check_load_trend());
        report.findings.extend(self.check_memory_performance());
        report.findings.extend(self.check_memory_fragmentation());
        report.findings.extend(self.check_disk_io());
        report.findings.extend(self.check_disk_iops());
        report.findings.extend(self.check_network_connections());
        report.findings.extend(self.check_network_latency());
        report.findings.extend(self.check_network_bandwidth());
        report.findings.extend(self.check_desktop_compositor());
        report.findings.extend(self.check_power());
        report.findings.extend(self.check_io_scheduler());

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

    fn is_slow(&self) -> bool {
        true // CPU 和磁盘 I/O 采样各需 1 秒
    }
}

/// 解析 /proc/stat 中的 CPU 时间
fn parse_cpu_stat(stat: &str) -> Option<(u64, u64)> {
    for line in stat.lines() {
        if line.starts_with("cpu ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 8 {
                return None;
            }
            // user nice system idle iowait irq softirq steal
            let values: Vec<u64> = parts[1..9]
                .iter()
                .filter_map(|v| v.parse().ok())
                .collect();
            if values.len() < 5 {
                return None;
            }
            let idle = values[3] + values[4]; // idle + iowait
            let total: u64 = values.iter().sum();
            return Some((idle, total));
        }
    }
    None
}

/// 获取默认网关
fn get_default_gateway() -> Option<String> {
    let output = Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    // 格式: default via 192.168.1.1 dev eth0
    stdout
        .split_whitespace()
        .nth(2)
        .map(|s| s.to_string())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cpu_stat_basic() {
        let stat = "cpu  1000 200 300 4000 500 60 70 80 0 0\n";
        let (idle, total) = parse_cpu_stat(stat).unwrap();
        assert_eq!(idle, 4500);
        assert_eq!(total, 6210);
    }

    #[test]
    fn parse_cpu_stat_no_cpu_line() {
        let stat = "intr 123456\nctxt 789\n";
        assert!(parse_cpu_stat(stat).is_none());
    }

    #[test]
    fn parse_cpu_stat_too_few_fields() {
        let stat = "cpu  100 200\n";
        assert!(parse_cpu_stat(stat).is_none());
    }

    #[test]
    fn parse_meminfo_basic() {
        let meminfo = "MemTotal:       16384000 kB\nMemAvailable:    8192000 kB\nSwapTotal:       2097152 kB\n";
        let map = parse_meminfo(meminfo);
        assert_eq!(map.get("MemTotal"), Some(&16384000));
        assert_eq!(map.get("MemAvailable"), Some(&8192000));
        assert_eq!(map.get("SwapTotal"), Some(&2097152));
        assert_eq!(map.get("NonExistent"), None);
    }

    #[test]
    fn parse_meminfo_empty() {
        let map = parse_meminfo("");
        assert!(map.is_empty());
    }

}
