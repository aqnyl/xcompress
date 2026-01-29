use crate::config::{self, FinalConfig};
use crate::utils::{is_restic_repo, run_restic_command};
use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password, Select};
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
        // 优先检查 backup_config.toml，其次检查 backup.toml
        let default_tomls = vec!["backup_config.toml", "backup.toml"];
        let mut found_toml = None;
        for t in default_tomls {
            if Path::new(t).exists() {
                found_toml = Some(t);
                break;
            }
        }
        
        if let Some(toml_file) = found_toml {
            println!("{} 检测到默认配置文件 '{}'，将使用该文件进行备份。", style("i").blue(), toml_file);
            match config::parse_toml(toml_file) {
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

pub fn handle_batch_backup(restic_exe_path: &str) -> Result<(), String> {
    println!("\n{}\n", style("--- 开始批量备份流程 ---").bold().yellow());
    let theme = ColorfulTheme::default();

    // 智能设置默认值
    let default_val = if Path::new("backup_config.toml").exists() {
        "backup_config.toml"
    } else if Path::new("backup.toml").exists() {
        "backup.toml"
    } else {
        "backup_config.toml"
    };

    // 1. Get TOML config path
    let config_path: String = Input::with_theme(&theme)
        .with_prompt("请输入或拖入 backup_config.toml 文件路径")
        .default(default_val.into())
        .validate_with(|input: &String| -> Result<(), &str> {
            if Path::new(input).exists() { Ok(()) } else { Err("文件不存在，请重新输入。") }
        })
        .interact_text()
        .map_err(|e| e.to_string())?;
    
    // 2. Parse TOML and run backups
    match config::parse_toml(&config_path) {
        Ok(configs) => {
            run_toml_backup(restic_exe_path, configs);
            Ok(())
        },
        Err(e) => Err(e),
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
    let backup_result = execute_backup(restic_exe_path, repo_path, &merge_path, &config.passwd, &config.tag, config.pack_size);

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
        match execute_backup(restic_exe_path, repo_path, backup_path, &config.passwd, &config.tag, config.pack_size) {
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

    // 1. Get path to back up
    let backup_path_str = match &target_path {
        Some(path) => path.clone(),
        None => Input::with_theme(&theme)
            .with_prompt("请输入或拖入要备份的文件/目录路径")
            .validate_with(|input: &String| -> Result<(), &str> {
                if Path::new(input).exists() { Ok(()) } else { Err("路径不存在，请重新输入。") }
            })
            .interact_text().map_err(|e| e.to_string())?,
    };
    let backup_path = Path::new(&backup_path_str);

    // --- 智能仓库路径选择 logic ---
    let mut repo_path_option: Option<PathBuf> = None;

    // 如果是通过拖拽/命令行直接传入路径，进入“快速模式”
    if target_path.is_some() {
        let parent_dir = backup_path.parent().unwrap_or(Path::new("."));
        let dir_name = backup_path.file_name().unwrap_or_default().to_string_lossy();
        // 默认推荐: 在源目录旁边创建一个 xxx_repo 的目录
        let suggested_repo_name = format!("{}_repo", dir_name);
        let suggested_repo_path = parent_dir.join(&suggested_repo_name);

        println!("\n{}", style("--- 快速备份模式 ---").cyan().bold());
        
        let opts = vec![
            format!("使用默认仓库位置: {} (推荐)", style(suggested_repo_path.display()).green()),
            "自定义位置 / 选择现有仓库".to_string()
        ];

        let sel = Select::with_theme(&theme)
            .with_prompt(format!("检测到源目录 '{}'，请选择备份方案", dir_name))
            .items(&opts)
            .default(0)
            .interact()
            .unwrap_or(0);

        if sel == 0 {
            repo_path_option = Some(suggested_repo_path);
        }
    }

    // 确定最终的 repo_path (解决编译器重复赋值/未初始化问题)
    let repo_path = if let Some(path) = repo_path_option {
        path
    } else {
        // --- 传统手动模式：选择 Base Dir -> 扫描/新建 ---
        let exe_dir = env::current_dir().map_err(|e| format!("获取当前目录失败: {}", e))?;
        let default_repo_base = exe_dir.to_string_lossy().to_string();
        let repo_base_str: String = Input::with_theme(&theme)
            .with_prompt("请输入 restic 仓库的存放目录 (留空则使用当前程序目录)")
            .default(default_repo_base)
            .interact_text()
            .map_err(|e| e.to_string())?;
        let repo_base_path = Path::new(&repo_base_str);

        if !repo_base_path.exists() || !repo_base_path.is_dir() {
            return Err(format!("仓库存放目录 '{}' 不是一个有效的目录。", repo_base_str));
        }

        println!("\n{} 正在扫描 '{}' 下的 restic 仓库...", style("i").blue(), repo_base_path.display());
        let mut existing_repos = Vec::new();
        if let Ok(entries) = fs::read_dir(repo_base_path) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() && is_restic_repo(&path) {
                    existing_repos.push(path);
                }
            }
        }

        let mut selection_items: Vec<String> = existing_repos
            .iter()
            .map(|p| p.file_name().unwrap_or_default().to_string_lossy().to_string())
            .collect();

        selection_items.sort();
        let create_new_option = "[ 创建一个新的备份仓库 ]".to_string();
        selection_items.push(create_new_option.clone());

        let selection = Select::with_theme(&theme)
            .with_prompt("请选择要使用的备份仓库")
            .items(&selection_items)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;

        if selection_items[selection] == create_new_option {
            loop {
                let backup_name: String = Input::with_theme(&theme)
                    .with_prompt("请输入新备份仓库的名称")
                    .interact_text().map_err(|e| e.to_string())?;
                let potential_path = repo_base_path.join(&backup_name);
                if potential_path.exists() {
                    eprintln!("{}", style(format!("错误: 目录 '{}' 已存在。", potential_path.display())).red());
                } else {
                    break potential_path;
                }
            }
        } else {
            repo_base_path.join(&selection_items[selection])
        }
    };

    // 6. Get password and confirm
    let password = Password::with_theme(&theme)
        .with_prompt(format!("请输入仓库 '{}' 的密码", repo_path.file_name().unwrap_or_default().to_string_lossy()))
        .with_confirmation("请再次输入密码确认", "两次输入的密码不匹配。")
        .interact().map_err(|e| e.to_string())?;

    // 7. 每次备份前询问 Pack Size (默认值修改为 128)
    let size_opts = vec![
        "最大 (128 MiB) - 文件少易上传网盘 / 压缩率最高 / 推荐 TB 级数据",
        "默认 (16 MiB) - 文件碎片较多 / 本地性能最高 / 压缩率中等",
        "自定义大小 (16-128 MiB)"
    ];
    let size_selection = Select::with_theme(&theme)
        .with_prompt("请选择本次备份的存储单元大小 (Pack Size)")
        .items(&size_opts)
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;

    let pack_size = match size_selection {
        0 => 128,
        1 => 16,
        2 => Input::with_theme(&theme)
            .with_prompt("请输入 Pack Size (MiB)")
            .default(128)
            .validate_with(|input: &u64| -> Result<(), &str> {
                if *input >= 16 && *input <= 128 { Ok(()) } else { Err("范围必须在 16 到 128 之间") }
            })
            .interact_text()
            .map_err(|e| e.to_string())?,
        _ => 128,
    };

    println!("\n{} 准备将 '{}' 备份到 '{}'...", style("i").blue(), backup_path.display(), repo_path.display());

    if !Confirm::with_theme(&theme).with_prompt("确认开始备份吗?").interact().unwrap_or(false) {
        println!("{}", style("操作已取消。").yellow());
        return Ok(());
    }

    // 8. Execute backup
    match execute_backup(restic_exe_path, &repo_path, backup_path, &password, "", pack_size) {
        Ok(msg) => {
            println!("{}\n{}", style("✔ 交互式备份成功!").green().bold(), msg);
            Ok(())
        },
        Err(e) => Err(e),
    }
}

/// 核心备份执行函数
fn execute_backup(restic_exe_path: &str, repo_path: &Path, backup_path: &Path, passwd: &str, tag: &str, pack_size: u64) -> Result<String, String> {
    // 1. 如果仓库不存在，则自动初始化
    if !is_restic_repo(repo_path) {
        if repo_path.exists() && repo_path.read_dir().unwrap().next().is_some() {
            return Err(format!("目录 {} 已存在但不是有效的 restic 仓库。", repo_path.display()));
        }
        fs::create_dir_all(repo_path).map_err(|e| format!("创建仓库目录失败: {}", e))?;
        
        println!("{} 仓库 {} 不存在，正在初始化...", style("i").blue(), repo_path.display());
        // init 时不强制指定 pack-size，留给 backup 命令指定
        let init_args = ["-r", &repo_path.to_string_lossy(), "init"];
        run_restic_command(restic_exe_path, &init_args, passwd)?;
        println!("{} 仓库初始化成功。", style("✔").green());
    }

    // 2. 执行备份
    let repo_path_str = repo_path.to_string_lossy();
    let backup_path_str = backup_path.to_string_lossy();
    let pack_size_str = pack_size.to_string();
    
    let mut backup_args = vec![
        "-r", &repo_path_str,
        "backup", &backup_path_str,
        "--no-scan",
        "--pack-size", &pack_size_str // 在备份时指定 pack-size
    ];

    if !tag.is_empty() {
        backup_args.push("--tag");
        backup_args.push(tag);
    }
    
    println!("{} 开始执行备份...", style("i").blue());
    run_restic_command(restic_exe_path, &backup_args, passwd)
}