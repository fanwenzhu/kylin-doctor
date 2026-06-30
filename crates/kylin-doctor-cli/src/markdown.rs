/// 简单的终端 Markdown 渲染器
///
/// 将 Markdown 文本转换为带 ANSI 颜色的终端输出
///
/// 支持的语法：
/// - 代码块：```language ... ```
/// - 行内代码：`code`
/// - 粗体：**text**
/// - 斜体：*text*
/// - 列表：- item 或 1. item
/// - 标题：# ## ###
/// - 链接：[text](url)
pub fn render_markdown(text: &str) -> String {
    let mut output = String::new();
    let mut in_code_block = false;
    let mut code_block_lang = String::new();

    for line in text.lines() {
        // 处理代码块
        if line.starts_with("```") {
            if in_code_block {
                // 结束代码块
                output.push_str("\x1b[0m"); // 重置颜色
                output.push('\n');
                in_code_block = false;
                code_block_lang.clear();
            } else {
                // 开始代码块
                in_code_block = true;
                code_block_lang = line[3..].trim().to_string();
                output.push_str("\x1b[48;5;236m"); // 深灰色背景
                output.push('\n');
            }
            continue;
        }

        if in_code_block {
            // 代码块内内容
            output.push_str(&format!("  {}\n", line));
            continue;
        }

        // 处理标题
        if line.starts_with("# ") {
            output.push_str(&format!("\x1b[1;36m{}\x1b[0m\n", &line[2..]));
            continue;
        }
        if line.starts_with("## ") {
            output.push_str(&format!("\x1b[1;35m{}\x1b[0m\n", &line[3..]));
            continue;
        }
        if line.starts_with("### ") {
            output.push_str(&format!("\x1b[1;34m{}\x1b[0m\n", &line[4..]));
            continue;
        }

        // 处理列表
        if line.starts_with("- ") || line.starts_with("* ") {
            output.push_str(&format!("  • {}\n", render_inline(&line[2..])));
            continue;
        }
        if line.len() > 2 && line.chars().nth(0).unwrap().is_ascii_digit() && line.chars().nth(1) == Some('.') {
            output.push_str(&format!("  {}\n", render_inline(line)));
            continue;
        }

        // 普通行
        output.push_str(&format!("{}\n", render_inline(line)));
    }

    output
}

/// 渲染行内元素
fn render_inline(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();
    let mut in_code = false;
    let mut in_bold = false;
    let mut in_italic = false;

    while let Some(c) = chars.next() {
        match c {
            '`' => {
                if in_code {
                    result.push_str("\x1b[0m");
                    in_code = false;
                } else {
                    result.push_str("\x1b[48;5;236m");
                    in_code = true;
                }
            }
            '*' if !in_code => {
                if chars.peek() == Some(&'*') {
                    // 粗体
                    chars.next(); // 消费第二个 *
                    if in_bold {
                        result.push_str("\x1b[0m");
                        in_bold = false;
                    } else {
                        result.push_str("\x1b[1m");
                        in_bold = true;
                    }
                } else {
                    // 斜体
                    if in_italic {
                        result.push_str("\x1b[0m");
                        in_italic = false;
                    } else {
                        result.push_str("\x1b[3m");
                        in_italic = true;
                    }
                }
            }
            '[' if !in_code => {
                // 链接 [text](url)
                let mut link_text = String::new();
                let mut link_url = String::new();

                // 收集链接文本
                while let Some(nc) = chars.next() {
                    if nc == ']' {
                        break;
                    }
                    link_text.push(nc);
                }

                // 检查是否有 (
                if chars.peek() == Some(&'(') {
                    chars.next(); // 消费 (
                    while let Some(nc) = chars.next() {
                        if nc == ')' {
                            break;
                        }
                        link_url.push(nc);
                    }
                    result.push_str(&format!("\x1b[4;34m{}\x1b[0m", link_text));
                } else {
                    // 不是链接，原样输出
                    result.push('[');
                    result.push_str(&link_text);
                    result.push(']');
                }
            }
            _ => {
                result.push(c);
            }
        }
    }

    // 确保关闭所有格式
    if in_code {
        result.push_str("\x1b[0m");
    }
    if in_bold {
        result.push_str("\x1b[0m");
    }
    if in_italic {
        result.push_str("\x1b[0m");
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_bold() {
        let result = render_inline("这是**粗体**文本");
        assert!(result.contains("\x1b[1m"));
        assert!(result.contains("粗体"));
    }

    #[test]
    fn test_render_inline_code() {
        let result = render_inline("这是`code`文本");
        assert!(result.contains("\x1b[48;5;236m"));
        assert!(result.contains("code"));
    }

    #[test]
    fn test_render_list() {
        let result = render_markdown("- 列表项1\n- 列表项2");
        assert!(result.contains("• 列表项1"));
        assert!(result.contains("• 列表项2"));
    }

    #[test]
    fn test_render_code_block() {
        let input = "```rust\nfn main() {\n    println!(\"hello\");\n}\n```";
        let result = render_markdown(input);
        assert!(result.contains("fn main()"));
    }
}
