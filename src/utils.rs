use std::env;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use console::{style, Term};
use indicatif::{ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref TERM: Term = Term::stdout();
}

/// 打印彩色标题
pub fn print_header() {
    let version = env!("CARGO_PKG_VERSION");
    let border = "=======================================================================";
    println!("\n{}\n", style(border).magenta());
    println!("{}", style(format!("        xcompress v{} - 您的 restic 备份/恢复助手", version)).cyan().bold());
    println!("        {}\n", style("作者: 菜玖玖emoji | Bilibili: space.bilibili.com/395819372").yellow());
    println!("{}\n", style(border).magenta());
}

/// 检查 restic 是否可用，并返回其路径
/// 检查顺序: 1. 程序同目录下的 restic/restic.exe; 2. 系统 PATH
/// 返回值:
/// - Ok(String): restic 的可执行路径
/// - Err(String): 未找到 restic 的错误信息
pub fn check_restic_path() -> Result<String, String> {
    // 优先查找当前程序所在目录下是否有 restic 可执行文件
    if let Ok(exe_path) = env::current_exe() {
        if let Some(script_dir) = exe_path.parent() {
            // 根据操作系统决定优先检查的文件名
            let binary_name = if cfg!(windows) { "restic.exe" } else { "restic" };
            let primary_path = script_dir.join(binary_name);
            
            // 兼容性检查：如果在 Linux 上放了 restic.exe 或者 Windows 上是 restic
            let secondary_name = if cfg!(windows) { "restic" } else { "restic.exe" };
            let secondary_path = script_dir.join(secondary_name);

            if primary_path.exists() {
                println!("{} {}", style("✔").green(), style(format!("检测到程序目录中的 {}，将优先使用。", binary_name)).dim());
                return Ok(primary_path.to_string_lossy().into_owned());
            } else if secondary_path.exists() {
                println!("{} {}", style("✔").green(), style(format!("检测到程序目录中的 {}，将优先使用。", secondary_name)).dim());
                return Ok(secondary_path.to_string_lossy().into_owned());
            }
        }
    }
    
    // 检查系统 PATH 中是否有 restic 命令
    let output = Command::new("restic").arg("version").output();
    if let Ok(output) = output {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let re = Regex::new(r"restic \d+\.").unwrap();
            // 只要包含 restic 版本信息即可
            if re.is_match(&stdout) || stdout.contains("restic") {
                println!("{} {}", style("✔").green(), style("检测到系统 PATH 中的 restic，将使用系统版本。").dim());
                return Ok("restic".to_string());
            }
        }
    }

    Err(format!(
        "{} {}",
        style("✖").red(),
        style("错误: 未找到 restic 环境。\n请将 restic 可执行文件(Linux下通常为 'restic', Windows下为 'restic.exe') 放置于本程序同目录下，或将其路径添加到系统 PATH 环境变量中。").red().bold()
    ))
}

/// 格式化字节大小为可读的字符串
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.2} TiB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GiB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MiB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KiB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// 将标准系统路径转换为 restic 在 restore <id>:"/path" 中接受的格式
/// Windows: D:\work\project -> /D/work/project
/// Linux: /home/user -> /home/user (无变化)
pub fn convert_to_restic_path(p: &Path) -> String {
    if cfg!(windows) {
        let mut path_str = p.to_string_lossy().to_string();
        // 替换反斜杠
        path_str = path_str.replace("\\", "/");
        // 处理驱动器号
        if let Some(drive_colon) = path_str.get(0..2) {
             if drive_colon.ends_with(':') {
                 // D: -> /D
                 return format!("/{}", path_str.replace(":", ""));
             }
        }
        // 如果是 UNC 路径或其它特殊格式，可能不会被转换，但这能处理绝大多数情况
        path_str
    } else {
        // 对于类 Unix 系统, 路径已经是正确的格式
        p.to_string_lossy().to_string()
    }
}


/// 严格检查给定的路径是否为一个有效的 restic 仓库
pub fn is_restic_repo(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let required_dirs = ["snapshots", "index", "data", "keys", "locks"];
    let required_files = ["config"];

    for dir in &required_dirs {
        if !path.join(dir).is_dir() {
            return false;
        }
    }

    for file in &required_files {
        if !path.join(file).is_file() {
            return false;
        }
    }
    true
}

/// 带有密码输入的 Restic 命令执行器
///
/// # 参数
/// - `restic_exe_path`: restic 可执行文件路径
/// - `args`: 传递给 restic 的参数列表
/// - `password`: 仓库密码
///
/// # 返回
/// - `Ok(String)`: 命令成功执行的标准输出
/// - `Err(String)`: 错误信息（包含标准错误输出）
pub fn run_restic_command(restic_exe_path: &str, args: &[&str], password: &str) -> Result<String, String> {
    let mut child = Command::new(restic_exe_path)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("启动 restic 进程失败: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(password.as_bytes())
             .and_then(|_| stdin.write_all(b"\n"))
             .map_err(|e| format!("向 restic 写入密码失败: {}", e))?;
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(ProgressStyle::default_spinner()
        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", " "])
        .template("{spinner:.cyan} {msg}").unwrap());
    spinner.set_message("正在执行 restic 命令，请稍候...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let output = child.wait_with_output().map_err(|e| format!("等待 restic 进程失败: {}", e))?;
    
    spinner.finish_and_clear();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    
    if output.status.success() {
        Ok(stdout)
    } else {
        // 提供更友好的错误提示
        if stderr.contains("wrong password or no key found") {
            Err("密码错误。".to_string())
        } else if stderr.contains("Is there a repository at the given location?") || stderr.contains("repository does not exist") {
            Err("仓库路径无效或不存在。".to_string())
        } else {
            Err(format!(
                "Restic 命令执行失败:\n--- 标准输出 ---\n{}\n--- 标准错误 ---\n{}",
                stdout, stderr
            ))
        }
    }
}