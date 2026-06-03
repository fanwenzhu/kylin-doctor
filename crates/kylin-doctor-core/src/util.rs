/// 读取 sysfs 中的 u64 值
pub fn read_sysfs_u64(path: std::path::PathBuf) -> u64 {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
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
}
