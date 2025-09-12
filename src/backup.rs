use crate::config::{self, FinalConfig};
use crate::utils::{run_restic_command, is_restic_repo};
use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn handle_backup(restic_exe_path: &str, config_path: Option<String>, target_path: Option<String>) {
    println!("\n{}\n", style("--- 开始备份流程 ---").bold().yellow());

    if let Some(path) = config_path {
        // 模式一：使用指定的 toml 配置文件
        match config::parse_toml(&path) {
            Ok(configs) => run_toml_backup(restic_exe_path, configs),
            Err(e) => eprintln!("{} {}", style("✖").red(), style(e).red().bold()),
        }
    } else if let Some(path) = target_path {
        // 模式二：直接备份指定的路径 (交互式)
        if !Path::new(&path).exists() {
             eprintln!("{} {}", style("✖").red(), style(format!("错误: 提供的路径 '{}' 不存在。", path)).red().bold());
        } else {
             run_interactive_backup(restic_exe_path, Some(path)).unwrap_or_else(|e| {
                 eprintln!("{} {}", style("✖").red(), style(format!("交互式备份失败: {}", e)).red().bold());
             });
        }
    } else {
        // 模式三：无参数，检查默认 toml 或进入交互式
        let default_toml = "backup_config.toml";
        if Path::new(default_toml).exists() {
             println!("{} 检测到默认配置文件 '{}'，将使用该文件进行备份。", style("i").blue(), default_toml);
             match config::parse_toml(default_toml) {
                Ok(configs) => run_toml_backup(restic_exe_path, configs),
                Err(e) => eprintln!("{} {}", style("✖").red(), style(e).red().bold()),
            }
        } else {
             println!("{} 未提供参数且未找到默认配置文件，进入交互式备份模式。", style("i").blue());
             run_interactive_backup(restic_exe_path, None).unwrap_or_else(|e| {
                 eprintln!("{} {}", style("✖").red(), style(format!("交互式备份失败: {}", e)).red().bold());
             });
        }
    }
}

fn run_toml_backup(restic_exe_path: &str, configs: Vec<FinalConfig>) {
    println!("{} 成功解析配置文件，共找到 {} 个备份任务。", style("✔").green(), configs.len());
    let mut summary = Vec::new();

    for config in configs {
        println!("\n{}", style(format!("--- 处理任务: {} ({}) ---", config.key_name, config.name)).cyan().bold());
        let final_repo_path = PathBuf::from(&config.restic_home_path).join(&config.name);
        println!("{} 仓库路径: {}", style("→").dim(), final_repo_path.display());
        
        let result = if config.merge == 1 {
            backup_merged(restic_exe_path, &config, &final_repo_path)
        } else {
            backup_individual(restic_exe_path, &config, &final_repo_path)
        };

        match result {
            Ok(msg) => summary.push(format!("{} {}: {}", style("✔").green(), config.key_name, msg)),
            Err(e) => summary.push(format!("{} {}: {}", style("✖").red(), config.key_name, e)),
        }
    }

    println!("\n\n{}\n{}", style("===== 备份汇总 =====").yellow().bold(), summary.join("\n"));
}

fn backup_merged(restic_exe_path: &str, config: &FinalConfig, repo_path: &Path) -> Result<String, String> {
    println!("{} 模式: 合并备份 (共 {} 个路径)", style("→").dim(), config.path.len());
    
    // 创建临时合并目录
    let temp_dir_name = format!("{}_{}", config.merge_name, SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs());
    let merge_path = env::temp_dir().join(temp_dir_name);
    fs::create_dir_all(&merge_path).map_err(|e| format!("创建合并目录失败: {}", e))?;

    // 复制文件/目录到合并目录
    let mut copy_errors = Vec::new();
    for src_path_str in &config.path {
        let src_path = Path::new(src_path_str);
        let dest_path = merge_path.join(src_path.file_name().unwrap_or_default());
        print!("  - 正在复制 {} 到 {} ... ", style(src_path.display()).dim(), style(dest_path.display()).dim());
        
        let result = if src_path.is_dir() {
            fs_extra::dir::copy(src_path, &merge_path, &fs_extra::dir::CopyOptions::new())
        } else {
            fs_extra::file::copy(src_path, &dest_path, &fs_extra::file::CopyOptions::new())
        };

        match result {
            Ok(_) => println!("{}", style("完成").green()),
            Err(e) => {
                let err_msg = format!("复制 {} 失败: {}", src_path_str, e);
                println!("{} {}", style("失败").red(), style(&err_msg).red());
                copy_errors.push(err_msg);
            }
        }
    }
    
    if !copy_errors.is_empty() {
        // 清理临时目录
        let _ = fs::remove_dir_all(&merge_path);
        return Err(format!("文件复制阶段出现错误:\n{}", copy_errors.join("\n")));
    }
    
    // 执行备份
    println!("{} 开始备份合并目录 {} ...", style("→").dim(), merge_path.display());
    let backup_result = execute_backup(restic_exe_path, repo_path, &merge_path, &config.passwd, &config.tag);

    // 清理临时目录
    let _ = fs::remove_dir_all(&merge_path);

    backup_result
}

fn backup_individual(restic_exe_path: &str, config: &FinalConfig, repo_path: &Path) -> Result<String, String> {
    println!("{} 模式: 单独备份 (共 {} 个路径)", style("→").dim(), config.path.len());
    let mut success_count = 0;
    let mut path_errors = Vec::new();

    for path_str in &config.path {
        let backup_path = Path::new(path_str);
        println!("  - 正在备份 {} ...", style(backup_path.display()).dim());
        match execute_backup(restic_exe_path, repo_path, backup_path, &config.passwd, &config.tag) {
            Ok(_) => success_count += 1,
            Err(e) => path_errors.push(format!("路径 {} 备份失败: {}", path_str, e)),
        }
    }
    
    if path_errors.is_empty() {
        Ok(format!("所有 {} 个路径单独备份成功。", success_count))
    } else {
        Err(format!("{}/{} 个路径备份成功，错误详情:\n{}", success_count, config.path.len(), path_errors.join("\n")))
    }
}

fn run_interactive_backup(restic_exe_path: &str, target_path: Option<String>) -> Result<(), String> {
    let theme = ColorfulTheme::default();
    
    let backup_path_str = match target_path {
        Some(path) => path,
        None => Input::with_theme(&theme)
            .with_prompt("请输入或拖入要备份的文件/目录路径")
            .validate_with(|input: &String| -> Result<(), &str> {
                if Path::new(input).exists() { Ok(()) } else { Err("路径不存在，请重新输入。") }
            })
            .interact_text().map_err(|e| e.to_string())?,
    };
    let backup_path = Path::new(&backup_path_str);
    
    let backup_name: String = Input::with_theme(&theme)
        .with_prompt("请输入备份名称 (将作为仓库目录名)")
        .interact_text().map_err(|e| e.to_string())?;

    let password = Password::with_theme(&theme)
        .with_prompt("请输入备份密码")
        .with_confirmation("请再次输入密码确认", "两次输入的密码不匹配。") // FIX: Changed .confirmation to .with_confirmation
        .interact().map_err(|e| e.to_string())?;
        
    let exe_dir = env::current_dir().map_err(|e| format!("获取当前目录失败: {}", e))?;
    let repo_path = exe_dir.join(&backup_name);
    
    println!("\n{} 准备将 '{}' 备份到 '{}'...", style("i").blue(), backup_path.display(), repo_path.display());

    if !Confirm::with_theme(&theme).with_prompt("确认开始备份吗?").interact().unwrap_or(false) {
        println!("{}", style("操作已取消。").yellow());
        return Ok(());
    }

    match execute_backup(restic_exe_path, &repo_path, backup_path, &password, "") {
        Ok(msg) => {
            println!("{}\n{}", style("✔ 交互式备份成功!").green().bold(), msg);
            Ok(())
        },
        Err(e) => Err(e),
    }
}

/// 核心备份执行函数
fn execute_backup(restic_exe_path: &str, repo_path: &Path, backup_path: &Path, passwd: &str, tag: &str) -> Result<String, String> {
    // 1. 如果仓库不存在，则自动初始化
    if !is_restic_repo(repo_path) {
        if repo_path.exists() && repo_path.read_dir().unwrap().next().is_some() {
             return Err(format!("目录 {} 已存在但不是有效的 restic 仓库。", repo_path.display()));
        }
        fs::create_dir_all(repo_path).map_err(|e| format!("创建仓库目录失败: {}", e))?;
        println!("{} 仓库 {} 不存在，正在初始化...", style("i").blue(), repo_path.display());
        let init_args = ["-r", &repo_path.to_string_lossy(), "init"];
        run_restic_command(restic_exe_path, &init_args, passwd)?;
        println!("{} 仓库初始化成功。", style("✔").green());
    }

    // 2. 执行备份
    // FIX: Create bindings to extend the lifetime of the temporary strings
    let repo_path_str = repo_path.to_string_lossy();
    let backup_path_str = backup_path.to_string_lossy();
    
    let mut backup_args = vec![
        "-r", &repo_path_str,
        "backup", &backup_path_str,
        "--no-scan" // 提升性能
    ];

    if !tag.is_empty() {
        backup_args.push("--tag");
        backup_args.push(tag);
    }
    
    println!("{} 开始执行备份...", style("i").blue());
    run_restic_command(restic_exe_path, &backup_args, passwd)
}