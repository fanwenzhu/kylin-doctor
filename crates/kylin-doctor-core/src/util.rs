use std::process::Command;
use std::time::Duration;

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
}
