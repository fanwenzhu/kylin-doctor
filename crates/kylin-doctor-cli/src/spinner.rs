use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// 终端动画 Spinner
///
/// 参考 Claude Code 的设计，在工具调用时显示简洁的状态动画
///
/// # Example
/// ```rust
/// use crate::spinner::Spinner;
///
/// let spinner = Spinner::new("正在扫描系统");
/// spinner.start();
/// // ... 执行耗时操作 ...
/// spinner.stop(true);  // 成功
/// // 或
/// spinner.stop(false); // 失败
/// ```
pub struct Spinner {
    message: String,
    running: Arc<AtomicBool>,
}

impl Spinner {
    /// 动画帧字符（使用 Braille 字符）
    const FRAMES: [&'static str; 4] = ["⠋", "⠙", "⠹", "⠸"];

    /// 创建新的 Spinner
    ///
    /// # Arguments
    /// * `message` - 显示的状态消息
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 启动动画（在后台线程运行）
    pub fn start(&self) {
        let running = self.running.clone();
        let message = self.message.clone();

        running.store(true, Ordering::SeqCst);

        thread::spawn(move || {
            let mut frame = 0;
            while running.load(Ordering::SeqCst) {
                print!("\r  {} {}", Self::FRAMES[frame % 4], message);
                io::stdout().flush().unwrap();
                frame += 1;
                thread::sleep(Duration::from_millis(100));
            }
        });
    }

    /// 停止动画并显示结果
    ///
    /// # Arguments
    /// * `success` - true 显示 ✅，false 显示 ❌
    pub fn stop(&self, success: bool) {
        self.running.store(false, Ordering::SeqCst);

        // 清除当前行并显示结果
        let icon = if success { "✅" } else { "❌" };
        print!("\r{}", " ".repeat(self.message.len() + 10)); // 清除动画
        io::stdout().flush().unwrap();
        println!("\r  {} {}", icon, self.message);
    }

    /// 停止动画并显示自定义结果
    ///
    /// # Arguments
    /// * `icon` - 自定义图标（如 "⚠️"）
    /// * `suffix` - 消息后缀
    #[allow(dead_code)]
    pub fn stop_with(&self, icon: &str, suffix: &str) {
        self.running.store(false, Ordering::SeqCst);

        // 清除当前行并显示结果
        print!("\r{}", " ".repeat(self.message.len() + 10)); // 清除动画
        io::stdout().flush().unwrap();
        println!("\r  {} {} {}", icon, self.message, suffix);
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        // 确保 Spinner 被清理时停止动画
        self.running.store(false, Ordering::SeqCst);
    }
}

/// 带 Spinner 的异步操作执行器
///
/// # Example
/// ```rust
/// use crate::spinner::with_spinner;
///
/// let result = with_spinner("正在诊断系统", async {
///     // 执行异步操作
///     Ok("扫描结果".to_string())
/// }).await?;
/// ```
#[allow(dead_code)]
pub async fn with_spinner<F, T, E>(message: &str, future: F) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
{
    let spinner = Spinner::new(message);
    spinner.start();

    match future.await {
        Ok(result) => {
            spinner.stop(true);
            Ok(result)
        }
        Err(e) => {
            spinner.stop(false);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spinner_creation() {
        let spinner = Spinner::new("测试消息");
        assert_eq!(spinner.message, "测试消息");
    }
}
