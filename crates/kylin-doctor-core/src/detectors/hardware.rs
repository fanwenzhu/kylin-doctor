use crate::detector::{Detector, Finding, FixAction, ScanReport, Severity};
use crate::util::{parse_diskstats, read_sysfs_u64};
use std::process::Command;
use std::time::Instant;

/// 硬件健康检测模块
pub struct HardwareDetector;

impl HardwareDetector {
    pub fn new() -> Self {
        Self
    }

    /// 检查 CPU 温度
    fn check_cpu_temperature(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 读取 /sys/class/thermal/thermal_zone*/temp
        let zones = match std::fs::read_dir("/sys/class/thermal") {
            Ok(entries) => entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with("thermal_zone")
                })
                .collect::<Vec<_>>(),
            Err(_) => return findings,
        };

        for zone in zones {
            let zone_path = zone.path();
            let temp_path = zone_path.join("temp");
            let type_path = zone_path.join("type");

            let temp_str = match std::fs::read_to_string(&temp_path) {
                Ok(s) => s.trim().to_string(),
                Err(_) => continue,
            };

            let temp_millideg: i32 = match temp_str.parse() {
                Ok(v) => v,
                Err(_) => continue,
            };

            let temp_c = temp_millideg as f64 / 1000.0;
            let zone_name = std::fs::read_to_string(&type_path)
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| zone_path.file_name().unwrap().to_string_lossy().to_string());

            if temp_c > 95.0 {
                findings.push(Finding {
                    id: format!(
                        "hw-cpu-temp-critical-{}",
                        zone_path.file_name().unwrap().to_string_lossy()
                    ),
                    module: "hardware".to_string(),
                    severity: Severity::Critical,
                    title: format!("CPU 温度过高 ({:.0}°C) - {}", temp_c, zone_name),
                    description: format!(
                        "温度传感器 {} 读数 {:.1}°C，已超过 95°C 临界值，可能导致系统关机保护或硬件损坏。",
                        zone_name, temp_c
                    ),
                    evidence: format!("zone={}, temp={} millidegrees", zone_name, temp_millideg),
                    fix: Some(FixAction {
                        description: "检查散热风扇和散热器".to_string(),
                        command: "echo '请检查 CPU 散热器是否正常工作，清理灰尘，更换硅脂'".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            } else if temp_c > 80.0 {
                findings.push(Finding {
                    id: format!(
                        "hw-cpu-temp-warning-{}",
                        zone_path.file_name().unwrap().to_string_lossy()
                    ),
                    module: "hardware".to_string(),
                    severity: Severity::Warning,
                    title: format!("CPU 温度偏高 ({:.0}°C) - {}", temp_c, zone_name),
                    description: format!(
                        "温度传感器 {} 读数 {:.1}°C，超过 80°C，建议关注散热状况。",
                        zone_name, temp_c
                    ),
                    evidence: format!("zone={}, temp={} millidegrees", zone_name, temp_millideg),
                    fix: Some(FixAction {
                        description: "检查散热状况".to_string(),
                        command: "echo '建议清理散热器灰尘，确保通风良好'".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            }
        }

        findings
    }

    /// 检查内存使用率
    fn check_memory_usage(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let output = match std::fs::read_to_string("/proc/meminfo") {
            Ok(s) => s,
            Err(_) => return findings,
        };

        let mut mem_total_kb: u64 = 0;
        let mut mem_available_kb: u64 = 0;

        for line in output.lines() {
            if line.starts_with("MemTotal:") {
                mem_total_kb = parse_meminfo_value(line);
            } else if line.starts_with("MemAvailable:") {
                mem_available_kb = parse_meminfo_value(line);
            }
        }

        if mem_total_kb == 0 {
            return findings;
        }

        let used_kb = mem_total_kb.saturating_sub(mem_available_kb);
        let usage_pct = (used_kb as f64 / mem_total_kb as f64) * 100.0;
        let total_mb = mem_total_kb / 1024;
        let used_mb = used_kb / 1024;
        let avail_mb = mem_available_kb / 1024;

        if usage_pct > 95.0 {
            findings.push(Finding {
                id: "hw-memory-critical".to_string(),
                module: "hardware".to_string(),
                severity: Severity::Critical,
                title: format!("内存使用率极高 ({:.0}%)", usage_pct),
                description: format!(
                    "总计 {} MB，已用 {} MB，可用 {} MB。内存即将耗尽，可能导致 OOM killer 终止进程。",
                    total_mb, used_mb, avail_mb
                ),
                evidence: format!(
                    "MemTotal={}kB MemAvailable={}kB Used={:.0}%",
                    mem_total_kb, mem_available_kb, usage_pct
                ),
                fix: Some(FixAction {
                    description: "关闭占用内存较多的程序".to_string(),
                    command: "echo '运行 htop 或 ps aux --sort=-%mem 查看内存占用最高的进程'".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: false,
            });
        } else if usage_pct > 85.0 {
            findings.push(Finding {
                id: "hw-memory-warning".to_string(),
                module: "hardware".to_string(),
                severity: Severity::Warning,
                title: format!("内存使用率偏高 ({:.0}%)", usage_pct),
                description: format!(
                    "总计 {} MB，已用 {} MB，可用 {} MB。建议关闭不必要的程序释放内存。",
                    total_mb, used_mb, avail_mb
                ),
                evidence: format!(
                    "MemTotal={}kB MemAvailable={}kB Used={:.0}%",
                    mem_total_kb, mem_available_kb, usage_pct
                ),
                fix: Some(FixAction {
                    description: "清理内存缓存".to_string(),
                    command: "sudo sh -c 'echo 3 > /proc/sys/vm/drop_caches'".to_string(),
                    risk_level: "low".to_string(),
                    ..Default::default()
                }),
                auto_fixable: true,
            });
        }

        findings
    }

    /// 检查磁盘健康状态 (SMART)
    fn check_disk_health(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 获取块设备列表
        let output = match Command::new("lsblk")
            .args(["-dpno", "NAME,TYPE"])
            .output()
        {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return findings,
        };

        for line in output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 || parts[1] != "disk" {
                continue;
            }
            let device = parts[0];

            // 尝试 smartctl 检测
            let smart_output = match Command::new("smartctl")
                .args(["-H", device])
                .output()
            {
                Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
                Err(_) => continue, // smartctl 未安装，跳过
            };

            if smart_output.contains("SMART overall-health self-assessment test result: FAILED") {
                findings.push(Finding {
                    id: format!(
                        "hw-disk-health-critical-{}",
                        device.replace('/', "_")
                    ),
                    module: "hardware".to_string(),
                    severity: Severity::Critical,
                    title: format!("磁盘 {} SMART 健康检查失败", device),
                    description: format!(
                        "磁盘 {} 的 SMART 自检未通过，磁盘可能存在物理故障，建议尽快备份数据。",
                        device
                    ),
                    evidence: smart_output
                        .lines()
                        .filter(|l| l.contains("result:") || l.contains("FAILED"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    fix: Some(FixAction {
                        description: "立即备份数据并更换磁盘".to_string(),
                        command: format!(
                            "echo '磁盘 {} SMART 检测失败，请立即备份重要数据！'",
                            device
                        ),
                        risk_level: "high".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            }
        }

        findings
    }

    /// 检查磁盘 I/O 饱和度（采样 1 秒计算 I/O 利用率）
    fn check_disk_io_errors(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

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

            // io_time_ms: 磁盘忙于 I/O 的毫秒数（字段 12，1 秒采样内最大 1000）
            let io_time_diff = d2.io_time_ms.saturating_sub(d1.io_time_ms);

            // I/O 利用率 = io_time / 采样间隔 * 100%（采样间隔 = 1000ms）
            let io_util_pct = (io_time_diff as f64 / 1000.0) * 100.0;

            if io_util_pct > 95.0 {
                findings.push(Finding {
                    id: format!("hw-disk-io-saturated-{}", device),
                    module: "hardware".to_string(),
                    severity: Severity::Warning,
                    title: format!("磁盘 {} I/O 接近饱和 ({:.0}%)", device, io_util_pct),
                    description: format!(
                        "设备 {} I/O 利用率 {:.1}%，磁盘几乎一直在忙，可能成为性能瓶颈。",
                        device, io_util_pct
                    ),
                    evidence: format!(
                        "device={} io_util={:.1}% io_time_diff={}ms",
                        device, io_util_pct, io_time_diff
                    ),
                    fix: Some(FixAction {
                        description: "检查 I/O 密集型进程".to_string(),
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

    /// 检查 GPU 信息
    fn check_gpu(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 方法 1: lspci 查找 VGA/3D 控制器
        let lspci_output = Command::new("lspci").args(["-nn"]).output();
        if let Ok(o) = lspci_output {
            let stdout = String::from_utf8_lossy(&o.stdout);
            for line in stdout.lines() {
                if !line.contains("VGA") && !line.contains("3D controller") && !line.contains("Display") {
                    continue;
                }

                // 提取 PCI 地址和设备名
                let pci_addr = line.split_whitespace().next().unwrap_or("");
                let gpu_name = line.splitn(2, ':').nth(1).unwrap_or(line).trim();

                // nvidia-smi 获取详细信息
                let nvidia_output = Command::new("nvidia-smi")
                    .args(["--query-gpu=name,driver_version,memory.total,memory.used,temperature.gpu", "--format=csv,noheader,nounits"])
                    .output();

                if let Ok(no) = nvidia_output {
                    let nvstdout = String::from_utf8_lossy(&no.stdout);
                    for nvline in nvstdout.lines() {
                        let parts: Vec<&str> = nvline.split(',').map(|s| s.trim()).collect();
                        if parts.len() >= 5 {
                            let name = parts[0];
                            let driver = parts[1];
                            let vram_total = parts[2];
                            let vram_used = parts[3];
                            let temp_str = parts[4];

                            let temp: i32 = temp_str.parse().unwrap_or(0);
                            let vram_pct: f64 = if let (Ok(used), Ok(total)) = (vram_used.parse::<f64>(), vram_total.parse::<f64>()) {
                                if total > 0.0 { used / total * 100.0 } else { 0.0 }
                            } else {
                                0.0
                            };

                            findings.push(Finding {
                                id: "hw-gpu-nvidia-info".to_string(),
                                module: "hardware".to_string(),
                                severity: Severity::Info,
                                title: format!("NVIDIA GPU: {} (驱动 {})", name, driver),
                                description: format!(
                                    "显存: {}/{} MB ({:.0}%)，温度: {}°C",
                                    vram_used, vram_total, vram_pct, temp
                                ),
                                evidence: format!(
                                    "pci={} name={} driver={} vram={}/{}MB temp={}°C",
                                    pci_addr, name, driver, vram_used, vram_total, temp
                                ),
                                fix: None,
                                auto_fixable: false,
                            });

                            if temp > 90 {
                                findings.push(Finding {
                                    id: "hw-gpu-temp-critical".to_string(),
                                    module: "hardware".to_string(),
                                    severity: Severity::Critical,
                                    title: format!("GPU 温度过高 ({}°C)", temp),
                                    description: format!("NVIDIA GPU 温度 {}°C，超过 90°C 临界值，可能导致降频或关机。", temp),
                                    evidence: format!("temp={}°C", temp),
                                    fix: Some(FixAction {
                                        description: "检查 GPU 散热".to_string(),
                                        command: "nvidia-smi -q -d TEMPERATURE".to_string(),
                                        risk_level: "low".to_string(),
                                        ..Default::default()
                                    }),
                                    auto_fixable: false,
                                });
                            } else if temp > 80 {
                                findings.push(Finding {
                                    id: "hw-gpu-temp-warning".to_string(),
                                    module: "hardware".to_string(),
                                    severity: Severity::Warning,
                                    title: format!("GPU 温度偏高 ({}°C)", temp),
                                    description: format!("NVIDIA GPU 温度 {}°C，建议关注散热状况。", temp),
                                    evidence: format!("temp={}°C", temp),
                                    fix: Some(FixAction {
                                        description: "检查 GPU 散热".to_string(),
                                        command: "nvidia-smi -q -d TEMPERATURE".to_string(),
                                        risk_level: "low".to_string(),
                                        ..Default::default()
                                    }),
                                    auto_fixable: false,
                                });
                            }

                            if vram_pct > 90.0 {
                                findings.push(Finding {
                                    id: "hw-gpu-vram-high".to_string(),
                                    module: "hardware".to_string(),
                                    severity: Severity::Warning,
                                    title: format!("GPU 显存使用率过高 ({:.0}%)", vram_pct),
                                    description: format!("显存 {}/{} MB，使用率 {:.0}%，可能导致性能下降。", vram_used, vram_total, vram_pct),
                                    evidence: format!("vram={}/{}MB pct={:.0}%", vram_used, vram_total, vram_pct),
                                    fix: Some(FixAction {
                                        description: "关闭 GPU 密集型应用".to_string(),
                                        command: "nvidia-smi".to_string(),
                                        risk_level: "low".to_string(),
                                        ..Default::default()
                                    }),
                                    auto_fixable: false,
                                });
                            }
                        }
                    }
                } else {
                    // 非 NVIDIA GPU，只记录基本信息
                    findings.push(Finding {
                        id: format!("hw-gpu-info-{}", pci_addr.replace(':', "_").replace('.', "_")),
                        module: "hardware".to_string(),
                        severity: Severity::Info,
                        title: format!("GPU: {}", gpu_name),
                        description: format!("PCI 设备 {}，{}", pci_addr, gpu_name),
                        evidence: line.to_string(),
                        fix: None,
                        auto_fixable: false,
                    });
                }
            }
        }

        findings
    }

    /// 检查 USB 外设
    fn check_usb_devices(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let output = match Command::new("lsusb").output() {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return findings,
        };

        let device_count = output.lines().filter(|l| !l.trim().is_empty()).count();

        // 检查是否有打印机/扫描仪
        let mut printers = Vec::new();
        let mut scanners = Vec::new();

        for line in output.lines() {
            let lower = line.to_lowercase();
            if lower.contains("printer") {
                printers.push(line.trim().to_string());
            }
            if lower.contains("scanner") {
                scanners.push(line.trim().to_string());
            }
        }

        // 检查 cups 打印服务状态（如果有打印机）
        if !printers.is_empty() {
            let cups_running = Command::new("systemctl")
                .args(["is-active", "cups"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
                .unwrap_or(false);

            if !cups_running {
                findings.push(Finding {
                    id: "hw-usb-printer-cups".to_string(),
                    module: "hardware".to_string(),
                    severity: Severity::Warning,
                    title: "检测到打印机但 CUPS 服务未运行".to_string(),
                    description: format!(
                        "发现 {} 个打印设备，但 CUPS 打印服务未启动，打印机可能无法使用。",
                        printers.len()
                    ),
                    evidence: printers.join("\n"),
                    fix: Some(FixAction {
                        description: "启动 CUPS 打印服务".to_string(),
                        command: "sudo systemctl start cups && sudo systemctl enable cups".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        // 检查 SANE 扫描仪服务（如果有扫描仪）
        if !scanners.is_empty() {
            let sane_running = Command::new("systemctl")
                .args(["is-active", "saned"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
                .unwrap_or(false);

            if !sane_running {
                findings.push(Finding {
                    id: "hw-usb-scanner-sane".to_string(),
                    module: "hardware".to_string(),
                    severity: Severity::Info,
                    title: "检测到扫描仪但 SANE 服务未运行".to_string(),
                    description: format!(
                        "发现 {} 个扫描设备，SANE 服务未启动。本地扫描通常不需要此服务，网络扫描需要。",
                        scanners.len()
                    ),
                    evidence: scanners.join("\n"),
                    fix: Some(FixAction {
                        description: "启动 SANE 服务（如需网络扫描）".to_string(),
                        command: "sudo systemctl start saned".to_string(),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: true,
                });
            }
        }

        // 检查 USB 设备数量异常（可能有设备枚举问题）
        if device_count > 20 {
            findings.push(Finding {
                id: "hw-usb-count-high".to_string(),
                module: "hardware".to_string(),
                severity: Severity::Info,
                title: format!("USB 设备数量较多 ({})", device_count),
                description: format!(
                    "系统检测到 {} 个 USB 设备，数量较多。如有 USB 设备识别问题，建议检查 USB 控制器。",
                    device_count
                ),
                evidence: format!("device_count={}", device_count),
                fix: None,
                auto_fixable: false,
            });
        }

        findings
    }

    /// 检查主板信息（BIOS、DMI）
    fn check_motherboard(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 从 /sys/class/dmi/id/ 读取主板信息
        let dmi = "/sys/class/dmi/id";
        let bios_vendor = std::fs::read_to_string(format!("{}/bios_vendor", dmi)).map(|s| s.trim().to_string()).unwrap_or_default();
        let bios_version = std::fs::read_to_string(format!("{}/bios_version", dmi)).map(|s| s.trim().to_string()).unwrap_or_default();
        let bios_date = std::fs::read_to_string(format!("{}/bios_date", dmi)).map(|s| s.trim().to_string()).unwrap_or_default();
        let board_name = std::fs::read_to_string(format!("{}/board_name", dmi)).map(|s| s.trim().to_string()).unwrap_or_default();
        let board_vendor = std::fs::read_to_string(format!("{}/board_vendor", dmi)).map(|s| s.trim().to_string()).unwrap_or_default();

        if !bios_version.is_empty() {
            findings.push(Finding {
                id: "hw-mobo-info".to_string(),
                module: "hardware".to_string(),
                severity: Severity::Info,
                title: format!("主板: {} {}", board_vendor, board_name),
                description: format!("BIOS: {} {} ({})", bios_vendor, bios_version, bios_date),
                evidence: format!(
                    "bios_vendor={} bios_version={} bios_date={} board_vendor={} board_name={}",
                    bios_vendor, bios_version, bios_date, board_vendor, board_name
                ),
                fix: None,
                auto_fixable: false,
            });
        }

        // 检查 BIOS 日期是否过旧（超过 5 年）
        if !bios_date.is_empty() {
            // 格式通常是 MM/DD/YYYY 或 YYYY-MM-DD
            let year: Option<i32> = if bios_date.contains('/') {
                bios_date.split('/').nth(2).and_then(|y| y.parse().ok())
            } else if bios_date.contains('-') {
                bios_date.split('-').next().and_then(|y| y.parse().ok())
            } else {
                bios_date.parse().ok()
            };

            if let Some(y) = year {
                let current_year = chrono_now_year();
                if current_year - y > 5 {
                    findings.push(Finding {
                        id: "hw-mobo-bios-old".to_string(),
                        module: "hardware".to_string(),
                        severity: Severity::Info,
                        title: format!("BIOS 版本较旧 ({}年发布)", y),
                        description: format!(
                            "BIOS 日期 {}，距今 {} 年。较旧的 BIOS 可能存在安全漏洞或兼容性问题。",
                            bios_date, current_year - y
                        ),
                        evidence: format!("bios_date={}", bios_date),
                        fix: Some(FixAction {
                            description: "检查是否有 BIOS 更新".to_string(),
                            command: format!("echo '请访问 {} 官网查看 {} 的最新 BIOS 更新'", board_vendor, board_name),
                            risk_level: "medium".to_string(),
                            ..Default::default()
                        }),
                        auto_fixable: false,
                    });
                }
            }
        }

        // 检查 CMOS 电池电压（如果 hwmon 暴露）
        if let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") {
            for entry in entries.filter_map(|e| e.ok()) {
                let hwmon_path = entry.path();
                if let Ok(name) = std::fs::read_to_string(hwmon_path.join("name")) {
                    let name = name.trim().to_lowercase();
                    // 查找电池相关的 hwmon
                    if name.contains("bat") || name.contains("acpi") {
                        // 尝试读取 in0（通常是电压输入）
                        if let Ok(in0) = std::fs::read_to_string(hwmon_path.join("in0_input")) {
                            if let Ok(voltage_mv) = in0.trim().parse::<u64>() {
                                // CMOS 电池正常电压约 3V (3000mV)，低于 2.7V 需要更换
                                if voltage_mv < 2700 && voltage_mv > 0 {
                                    findings.push(Finding {
                                        id: "hw-mobo-cmos-battery-low".to_string(),
                                        module: "hardware".to_string(),
                                        severity: Severity::Warning,
                                        title: format!("CMOS 电池电压偏低 ({}mV)", voltage_mv),
                                        description: format!(
                                            "CMOS 电池电压 {}mV，低于正常值 (3000mV)。电池电量不足可能导致 BIOS 设置丢失、系统时间不准。",
                                            voltage_mv
                                        ),
                                        evidence: format!("hwmon={} in0={}mV", hwmon_path.display(), voltage_mv),
                                        fix: Some(FixAction {
                                            description: "更换 CMOS 电池".to_string(),
                                            command: "echo '请更换主板上的 CR2032 纽扣电池'".to_string(),
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

    /// 检查磁盘剩余寿命（SMART 属性）
    fn check_disk_lifespan(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let output = match Command::new("lsblk")
            .args(["-dpno", "NAME,TYPE"])
            .output()
        {
            Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
            Err(_) => return findings,
        };

        for line in output.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 || parts[1] != "disk" {
                continue;
            }
            let device = parts[0];

            let smart_output = match Command::new("smartctl")
                .args(["-A", device])
                .output()
            {
                Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
                Err(_) => continue,
            };

            // 查找寿命相关属性
            // SSD: Wear_Leveling_Count (ID 177/0xB1) 或 Media_Wearout_Indicator (ID 231/0xE7)
            // HDD: Reallocated_Sector_Ct (ID 5) 或 Seek_Error_Rate (ID 7)
            let mut wear_pct: Option<u32> = None;
            let mut reallocated_sectors: Option<u64> = None;

            for sline in smart_output.lines() {
                let fields: Vec<&str> = sline.split_whitespace().collect();
                if fields.len() < 10 {
                    continue;
                }

                let attr_id = fields[0];
                let attr_name = fields[1];
                let raw_value = fields.last().unwrap_or(&"");

                // SSD 寿命
                if attr_name == "Wear_Leveling_Count"
                    || attr_name == "Media_Wearout_Indicator"
                    || attr_name == "SSD_Life_Left"
                    || attr_id == "177"
                    || attr_id == "231"
                {
                    // VALUE 列 (fields[3]) 是归一化值，通常 100=全新, 0=寿命耗尽
                    if let Ok(val) = fields[3].parse::<u32>() {
                        wear_pct = Some(val);
                    }
                }

                // HDD 重映射扇区
                if attr_name == "Reallocated_Sector_Ct" || attr_id == "5" {
                    if let Ok(val) = raw_value.parse::<u64>() {
                        reallocated_sectors = Some(val);
                    }
                }
            }

            // 报告 SSD 寿命
            if let Some(pct) = wear_pct {
                let severity = if pct <= 10 {
                    Severity::Critical
                } else if pct <= 30 {
                    Severity::Warning
                } else {
                    Severity::Info
                };

                if pct <= 30 {
                    findings.push(Finding {
                        id: format!("hw-disk-wear-{}", device.replace('/', "_")),
                        module: "hardware".to_string(),
                        severity,
                        title: format!("磁盘 {} 剩余寿命 {}%", device, pct),
                        description: format!(
                            "设备 {} SMART Wear_Leveling 值为 {}%，{}。",
                            device,
                            pct,
                            if pct <= 10 { "寿命即将耗尽，建议立即更换" } else { "寿命偏低，建议关注并准备更换" }
                        ),
                        evidence: format!("device={} wear={}% raw={}", device, pct,
                            raw_smart_value(&smart_output, "Wear_Leveling_Count")
                                .or_else(|| raw_smart_value(&smart_output, "Media_Wearout_Indicator"))
                                .unwrap_or_default()
                        ),
                        fix: Some(FixAction {
                            description: if pct <= 10 { "立即备份数据并更换磁盘" } else { "计划更换磁盘" }.to_string(),
                            command: format!("sudo smartctl -a {}", device),
                            risk_level: "low".to_string(),
                            ..Default::default()
                        }),
                        auto_fixable: false,
                    });
                }
            }

            // 报告 HDD 重映射扇区
            if let Some(sectors) = reallocated_sectors {
                if sectors > 0 {
                    let severity = if sectors > 100 {
                        Severity::Critical
                    } else {
                        Severity::Warning
                    };
                    findings.push(Finding {
                        id: format!("hw-disk-reallocated-{}", device.replace('/', "_")),
                        module: "hardware".to_string(),
                        severity,
                        title: format!("磁盘 {} 存在重映射扇区 ({})", device, sectors),
                        description: format!(
                            "设备 {} 有 {} 个重映射扇区，表明磁盘表面有坏道。数量持续增长意味着磁盘即将故障。",
                            device, sectors
                        ),
                        evidence: format!("device={} reallocated_sectors={}", device, sectors),
                        fix: Some(FixAction {
                            description: "备份数据并监控坏道增长".to_string(),
                            command: format!("sudo smartctl -a {} | grep -i reallocated", device),
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

    /// 检查磁盘读写速度
    fn check_disk_speed(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        // 使用 /proc/diskstats 采样 1 秒计算近似速度
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

            let sectors_read = d2.sectors_read.saturating_sub(d1.sectors_read);
            let sectors_written = d2.sectors_written.saturating_sub(d1.sectors_written);

            // 扇区通常为 512 字节
            let read_mb_s = (sectors_read as f64 * 512.0) / (1024.0 * 1024.0);
            let write_mb_s = (sectors_written as f64 * 512.0) / (1024.0 * 1024.0);

            // 只在有明显 I/O 时报告
            if read_mb_s > 1.0 || write_mb_s > 1.0 {
                findings.push(Finding {
                    id: format!("hw-disk-speed-{}", device),
                    module: "hardware".to_string(),
                    severity: Severity::Info,
                    title: format!("磁盘 {} 读写速度: {:.1}/{:.1} MB/s", device, read_mb_s, write_mb_s),
                    description: format!(
                        "设备 {} 采样 1 秒内：读 {:.1} MB/s，写 {:.1} MB/s。",
                        device, read_mb_s, write_mb_s
                    ),
                    evidence: format!(
                        "device={} read={:.1}MB/s write={:.1}MB/s sectors_read={} sectors_written={}",
                        device, read_mb_s, write_mb_s, sectors_read, sectors_written
                    ),
                    fix: None,
                    auto_fixable: false,
                });
            }
        }

        findings
    }

    /// 检查网卡链路状态和错误
    fn check_network_interfaces(&self) -> Vec<Finding> {
        let mut findings = Vec::new();

        let net_dir = match std::fs::read_dir("/sys/class/net") {
            Ok(d) => d,
            Err(_) => return findings,
        };

        for entry in net_dir.filter_map(|e| e.ok()) {
            let iface = entry.file_name().to_string_lossy().to_string();

            // 跳过 lo 回环
            if iface == "lo" {
                continue;
            }

            let iface_path = entry.path();

            // 检查链路状态
            let operstate = std::fs::read_to_string(iface_path.join("operstate"))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();

            // 只对物理网卡检查（有 device 子目录的）
            let is_physical = iface_path.join("device").exists();

            if is_physical && operstate == "down" {
                findings.push(Finding {
                    id: format!("hw-nic-down-{}", iface),
                    module: "hardware".to_string(),
                    severity: Severity::Info,
                    title: format!("网卡 {} 链路断开", iface),
                    description: format!(
                        "物理网卡 {} 当前状态为 down，可能是网线未连接或交换机端口故障。",
                        iface
                    ),
                    evidence: format!("interface={} operstate={}", iface, operstate),
                    fix: Some(FixAction {
                        description: "检查网线连接".to_string(),
                        command: format!("sudo ip link set {} up", iface),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            }

            // 检查网卡错误计数
            let rx_errors = read_sysfs_u64(iface_path.join("statistics/rx_errors"));
            let tx_errors = read_sysfs_u64(iface_path.join("statistics/tx_errors"));
            let rx_dropped = read_sysfs_u64(iface_path.join("statistics/rx_dropped"));
            let tx_dropped = read_sysfs_u64(iface_path.join("statistics/tx_dropped"));

            let total_errors = rx_errors + tx_errors + rx_dropped + tx_dropped;

            if total_errors > 100 {
                findings.push(Finding {
                    id: format!("hw-nic-errors-{}", iface),
                    module: "hardware".to_string(),
                    severity: Severity::Warning,
                    title: format!("网卡 {} 存在大量错误/丢包 ({})", iface, total_errors),
                    description: format!(
                        "网卡 {} 错误统计：rx_errors={}, tx_errors={}, rx_dropped={}, tx_dropped={}。可能指示网卡故障或驱动问题。",
                        iface, rx_errors, tx_errors, rx_dropped, tx_dropped
                    ),
                    evidence: format!(
                        "interface={} rx_errors={} tx_errors={} rx_dropped={} tx_dropped={}",
                        iface, rx_errors, tx_errors, rx_dropped, tx_dropped
                    ),
                    fix: Some(FixAction {
                        description: "检查网卡驱动和硬件".to_string(),
                        command: format!("ethtool {} && dmesg | grep -i {}", iface, iface),
                        risk_level: "low".to_string(),
                        ..Default::default()
                    }),
                    auto_fixable: false,
                });
            }
        }

        findings
    }
}

impl Default for HardwareDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for HardwareDetector {
    fn name(&self) -> &str {
        "hardware"
    }

    fn description(&self) -> &str {
        "硬件健康检测 — CPU/GPU 温度、内存、磁盘健康/寿命/速度、网卡、USB 外设、主板信息"
    }

    fn scan(&self) -> anyhow::Result<ScanReport> {
        let start = Instant::now();
        let mut report = ScanReport::new("hardware".to_string());

        report.findings.extend(self.check_cpu_temperature());
        report.findings.extend(self.check_gpu());
        report.findings.extend(self.check_memory_usage());
        report.findings.extend(self.check_disk_health());
        report.findings.extend(self.check_disk_lifespan());
        report.findings.extend(self.check_disk_io_errors());
        report.findings.extend(self.check_disk_speed());
        report.findings.extend(self.check_network_interfaces());
        report.findings.extend(self.check_usb_devices());
        report.findings.extend(self.check_motherboard());

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
        true // check_disk_speed() 需要 1 秒采样
    }
}

/// 解析 /proc/meminfo 中的值（单位 kB）
fn parse_meminfo_value(line: &str) -> u64 {
    line.split_whitespace()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// 从 smartctl -A 输出中提取指定属性的 RAW_VALUE
fn raw_smart_value(smart_output: &str, attr_name: &str) -> Option<String> {
    for line in smart_output.lines() {
        if line.contains(attr_name) {
            // 格式: ID ATTR_NAME FLAG VALUE WORST THRESH TYPE UPDATED WHEN_FAILED RAW_VALUE
            // RAW_VALUE 是最后一列，但有些属性有多个词，所以取最后一个
            return line.split_whitespace().last().map(|s| s.to_string());
        }
    }
    None
}

/// 获取当前年份（从系统时间）
fn chrono_now_year() -> i32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| {
            // 简单计算: 一年 ≈ 365.2425 天
            let days = d.as_secs() / 86400;
            // 1970-01-01 起始
            let mut year = 1970;
            let mut remaining_days = days;
            loop {
                let days_in_year = if is_leap_year(year) { 366 } else { 365 };
                if remaining_days < days_in_year {
                    break;
                }
                remaining_days -= days_in_year;
                year += 1;
            }
            year as i32
        })
        .unwrap_or(2026) // 回退值
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_meminfo_value_basic() {
        assert_eq!(parse_meminfo_value("MemTotal:       16384000 kB"), 16384000);
    }

    #[test]
    fn parse_meminfo_value_empty() {
        assert_eq!(parse_meminfo_value(""), 0);
    }

    #[test]
    fn parse_meminfo_value_no_number() {
        assert_eq!(parse_meminfo_value("SomeKey:"), 0);
    }

    #[test]
    fn raw_smart_value_found() {
        let output = "ID# ATTRIBUTE_NAME          FLAGS    VALUE WORST THRESH FAIL RAW_VALUE\n  5 Reallocated_Sector_Ct   0x0033   100   100   010    -    0\n  9 Power_On_Hours          0x0032   097   097   000    -    12345\n";
        assert_eq!(raw_smart_value(output, "Power_On_Hours"), Some("12345".to_string()));
    }

    #[test]
    fn raw_smart_value_not_found() {
        assert_eq!(raw_smart_value("ID# ATTRIBUTE_NAME\n", "Missing"), None);
    }
}
