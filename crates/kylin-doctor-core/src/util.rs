use std::process::Command;
use std::time::Duration;

/// 清理 API 错误响应中的敏感信息（API Key 等）
pub fn sanitize_api_error(body: &str) -> String {
    let mut result = body.to_string();
    // 替换常见的 API Key 模式
    // OpenAI/Anthropic: sk-...
    if let Some(start) = result.find("sk-") {
        // 找到 sk- 后面的连续非空白字符
        let end = result[start + 3..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '}' || c == ']')
            .map(|i| start + 3 + i)
            .unwrap_or(result.len());
        let key_len = end - start;
        if key_len > 10 {
            // 只替换足够长的 key
            let masked = format!("sk-***已隐藏***");
            result = format!("{}{}{}", &result[..start], masked, &result[end..]);
        }
    }
    // Bearer token
    if let Some(start) = result.find("Bearer ") {
        let end = result[start + 7..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '}' || c == ']')
            .map(|i| start + 7 + i)
            .unwrap_or(result.len());
        let token_len = end - start - 7;
        if token_len > 10 {
            result = format!("{}Bearer ***已隐藏***{}", &result[..start], &result[end..]);
        }
    }
    // 截断过长的响应体
    if result.len() > 500 {
        result.truncate(500);
        result.push_str("... (已截断)");
    }
    result
}

/// 获取当前时间的 Unix 时间戳字符串（秒）
pub fn epoch_secs() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_else(|_| "unknown".to_string())
}

/// 读取 sysfs 中的 u64 值
pub fn read_sysfs_u64(path: std::path::PathBuf) -> u64 {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// 带超时的命令执行，防止外部命令挂起导致整个扫描卡住
/// 超时后子进程会被 kill，返回 None
pub fn command_output_with_timeout(cmd: &mut Command, timeout: Duration) -> Option<std::process::Output> {
    let mut child = match cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return None,
    };

    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => {
                // 进程已结束，收集输出
                return child.wait_with_output().ok();
            }
            Ok(None) => {
                // 进程还在运行，检查是否超时
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait(); // 回收子进程，避免僵尸进程
                    return None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return None,
        }
    }
}

/// 默认超时时间（秒）— 适用于大多数快速系统命令
pub const DEFAULT_CMD_TIMEOUT_SECS: u64 = 10;

/// 较长超时时间（秒）— 适用于可能较慢的命令（apt 等）
pub const LONG_CMD_TIMEOUT_SECS: u64 = 30;

/// /proc/diskstats 完整解析结果
///
/// 字段顺序参考内核文档: major minor name rd_ios rd_merges rd_sectors rd_time wr_ios wr_merges wr_sectors wr_time io_in_progress io_time weighted_io_time
#[derive(Debug, Clone, Default)]
pub struct DiskStats {
    pub reads_completed: u64,
    pub writes_completed: u64,
    pub sectors_read: u64,
    pub sectors_written: u64,
    pub read_time_ms: u64,
    pub write_time_ms: u64,
    pub io_time_ms: u64,
    pub io_in_progress: u64,
}

/// 解析 /proc/meminfo 内容，返回 key -> value(kB) 的映射
pub fn parse_meminfo(meminfo: &str) -> std::collections::HashMap<String, u64> {
    let mut map = std::collections::HashMap::new();
    for line in meminfo.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let key = parts[0].trim_end_matches(':').to_string();
            if let Ok(val) = parts[1].parse::<u64>() {
                map.insert(key, val);
            }
        }
    }
    map
}

/// 解析 /proc/diskstats 内容，返回 device_name -> DiskStats 的映射
pub fn parse_diskstats(content: &str) -> std::collections::HashMap<String, DiskStats> {
    let mut map = std::collections::HashMap::new();
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 14 {
            continue;
        }
        let device = parts[2].to_string();
        map.insert(
            device,
            DiskStats {
                reads_completed: parts[3].parse().unwrap_or(0),
                writes_completed: parts[7].parse().unwrap_or(0),
                sectors_read: parts[5].parse().unwrap_or(0),
                sectors_written: parts[9].parse().unwrap_or(0),
                read_time_ms: parts[6].parse().unwrap_or(0),
                write_time_ms: parts[10].parse().unwrap_or(0),
                io_time_ms: parts[12].parse().unwrap_or(0),
                io_in_progress: parts[11].parse().unwrap_or(0),
            },
        );
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn read_sysfs_u64_valid() {
        let dir = std::env::temp_dir().join("kylin-doctor-util-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_value");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "12345").unwrap();

        assert_eq!(read_sysfs_u64(path), 12345);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_sysfs_u64_with_whitespace() {
        let dir = std::env::temp_dir().join("kylin-doctor-util-test-ws");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_value");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "  67890  ").unwrap();

        assert_eq!(read_sysfs_u64(path), 67890);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_sysfs_u64_nonexistent() {
        let path = std::path::PathBuf::from("/nonexistent/path/that/does/not/exist");
        assert_eq!(read_sysfs_u64(path), 0);
    }

    #[test]
    fn read_sysfs_u64_invalid_content() {
        let dir = std::env::temp_dir().join("kylin-doctor-util-test-invalid");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_value");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "not_a_number").unwrap();

        assert_eq!(read_sysfs_u64(path), 0);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parse_diskstats_basic() {
        let diskstats = "   8       0 sda 1000 200 8000 300 500 100 4000 200 5 400 500\n";
        let map = parse_diskstats(diskstats);
        let sda = map.get("sda").unwrap();
        assert_eq!(sda.reads_completed, 1000);
        assert_eq!(sda.writes_completed, 500);
        assert_eq!(sda.sectors_read, 8000);
        assert_eq!(sda.sectors_written, 4000);
        assert_eq!(sda.read_time_ms, 300);
        assert_eq!(sda.write_time_ms, 200);
        assert_eq!(sda.io_time_ms, 400);
        assert_eq!(sda.io_in_progress, 5);
    }

    #[test]
    fn parse_diskstats_too_few_fields() {
        let diskstats = "   8       0 sda 1000 200\n";
        let map = parse_diskstats(diskstats);
        assert!(map.is_empty());
    }

    #[test]
    fn parse_diskstats_empty() {
        let map = parse_diskstats("");
        assert!(map.is_empty());
    }

    #[test]
    fn sanitize_api_error_masks_sk_key() {
        let body = r#"{"error":"Invalid API key: sk-abc123def456ghi789jkl012mno"}"#;
        let sanitized = sanitize_api_error(body);
        assert!(!sanitized.contains("sk-abc123def456ghi789jkl012mno"));
        assert!(sanitized.contains("sk-***已隐藏***"));
    }

    #[test]
    fn sanitize_api_error_masks_bearer_token() {
        let body = "Authorization failed: Bearer sk-ant-api-key-1234567890abcdef";
        let sanitized = sanitize_api_error(body);
        assert!(!sanitized.contains("sk-ant-api-key-1234567890abcdef"));
        assert!(sanitized.contains("***已隐藏***"));
    }

    #[test]
    fn sanitize_api_error_no_key() {
        let body = "Connection refused";
        let sanitized = sanitize_api_error(body);
        assert_eq!(sanitized, "Connection refused");
    }

    #[test]
    fn sanitize_api_error_short_sk_not_masked() {
        let body = "sky is blue";
        let sanitized = sanitize_api_error(body);
        assert!(sanitized.contains("sky is blue"));
    }

    #[test]
    fn sanitize_api_error_truncates_long_body() {
        let body = "x".repeat(600);
        let sanitized = sanitize_api_error(&body);
        assert!(sanitized.len() < 600);
        assert!(sanitized.contains("已截断"));
    }
}
