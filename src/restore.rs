use crate::config;
use crate::utils::{self, run_restic_command};
use console::style;
use dialoguer::{theme::ColorfulTheme, Input, Password, Select};
use regex::Regex;
use serde_json::Value;
use std::env;
use std::path::Path;

struct Snapshot {
    short_id: String,
    time: String,
    paths: Vec<String>,
    size: u64,
}

pub fn handle_restore(restic_exe_path: &str, repo_path_arg: Option<String>) -> Result<(), String> {
    println!("\n{}\n", style("--- 开始恢复流程 ---").bold().yellow());
    
    let theme = ColorfulTheme::default();
    
    // 如果命令行已提供路径，则使用它，否则提示用户输入
    let repo_path_str: String = match repo_path_arg {
        Some(path) => {
            println!("{} 使用命令行提供的仓库路径: {}", style("✔").green(), style(&path).dim());
            path
        }
        None => Input::with_theme(&theme)
            .with_prompt("请输入或拖入 restic 仓库路径")
            .interact_text()
            .map_err(|e| e.to_string())?,
    };
    
    let repo_path = Path::new(repo_path_str.trim());
    if !utils::is_restic_repo(repo_path) {
        return Err("提供的路径不是一个有效的 restic 仓库。".to_string());
    }

    // 获取密码
    let password = Password::with_theme(&theme)
        .with_prompt("请输入仓库密码")
        .interact()
        .map_err(|e| e.to_string())?;

    // 获取快照列表
    println!("\n{} 正在获取快照列表...", style("i").blue());
    let snapshots = get_snapshots(restic_exe_path, &repo_path.to_string_lossy(), &password)?;
    
    if snapshots.is_empty() {
        return Err("仓库中未找到任何快照。".to_string());
    }

    // 1. 让用户选择快照 (显示更详细信息)
    let snapshot_items: Vec<String> = snapshots
        .iter()
        .map(|s| {
            format!(
                "{}  ({})  {}  [{}]",
                s.short_id,
                s.time.split('T').next().unwrap_or(""),
                style(utils::format_bytes(s.size)).dim(),
                s.paths.join(", ")
            )
        })
        .collect();
    
    let selection_idx = match Select::with_theme(&theme)
        .with_prompt("请选择要恢复的快照 (按 'q' 退出)")
        .items(&snapshot_items)
        .default(0)
        .interact_opt()
        .map_err(|e| e.to_string())?
    {
        Some(index) => index,
        None => {
            println!("{}", style("操作已取消。").yellow());
            return Ok(());
        }
    };
    let selected_snapshot = &snapshots[selection_idx];

    // 2. 如果快照有多个路径，让用户选择一个
    let path_to_restore: &str = if selected_snapshot.paths.len() > 1 {
        let path_selection = Select::with_theme(&theme)
            .with_prompt("此快照包含多个路径，请选择要恢复哪一个")
            .items(&selected_snapshot.paths)
            .default(0)
            .interact()
            .map_err(|e| e.to_string())?;
        &selected_snapshot.paths[path_selection]
    } else if let Some(path) = selected_snapshot.paths.first() {
        path
    } else {
        return Err("此快照不包含任何可恢复的路径。".to_string());
    };

    // 3. 让用户选择恢复模式
    let restore_modes = &[
        "仅恢复最后一级目录 (推荐, 类似解压)",
        "按原始完整路径恢复 (restic 默认行为)",
    ];
    let mode_selection = Select::with_theme(&theme)
        .with_prompt("请选择恢复模式")
        .items(restore_modes)
        .default(0)
        .interact()
        .map_err(|e| e.to_string())?;

    // 4. 决定输出路径
    // 智能推断默认恢复路径: 优先使用仓库的上级目录 (模拟原地解压)
    let default_output_path = if let Some(parent) = repo_path.parent() {
        parent.to_string_lossy().to_string()
    } else {
        env::current_dir().map_err(|e| e.to_string())?.to_string_lossy().to_string()
    };

    let output_path_str: String = Input::with_theme(&theme)
        .with_prompt("请输入恢复目标路径 (默认恢复到仓库同级目录)")
        .default(default_output_path)
        .interact_text()
        .map_err(|e| e.to_string())?;
    
    println!("\n{} 準備恢復快照 {} 到 '{}'...", style("i").blue(), selected_snapshot.short_id, output_path_str);
    
    // 5. 构建恢复命令参数
    let repo_path_lossy = repo_path.to_string_lossy();
    let mut args_vec = vec!["-r", &repo_path_lossy, "restore"];
    
    let snapshot_arg: String;
    if mode_selection == 0 { // 模式: 剥离路径
        let original_path = Path::new(path_to_restore);
        if let Some(parent) = original_path.parent() {
            let restic_parent_path = utils::convert_to_restic_path(parent);
            snapshot_arg = format!("{}:{}", selected_snapshot.short_id, restic_parent_path);
            args_vec.push(&snapshot_arg);
        } else {
            args_vec.push(&selected_snapshot.short_id);
        }
    } else { // 模式: 完整路径
        args_vec.push(&selected_snapshot.short_id);
    }

    args_vec.extend(&["--target", &output_path_str]);
    
    match run_restic_command(restic_exe_path, &args_vec, &password) {
        Ok(output) => {
            println!("{}\n{}", style("✔ 恢复成功!").green().bold(), output);
            Ok(())
        },
        Err(e) => Err(format!("恢复失败: {}", e)),
    }
}

pub fn handle_batch_restore(restic_exe_path: &str) -> Result<(), String> {
    println!("\n{}\n", style("--- 开始批量恢复流程 ---").bold().yellow());
    let theme = ColorfulTheme::default();

    let config_path: String = Input::with_theme(&theme)
        .with_prompt("请输入或拖入 restore_config.toml 文件路径")
        .default("restore_config.toml".into())
        .validate_with(|input: &String| -> Result<(), &str> {
            if Path::new(input).exists() { Ok(()) } else { Err("文件不存在，请重新输入。") }
        })
        .interact_text()
        .map_err(|e| e.to_string())?;

    let configs = config::parse_restore_toml(&config_path)?;

    println!("{} 成功解析恢复配置文件，共找到 {} 个恢复任务。", style("✔").green(), configs.len());
    let mut summary = Vec::new();

    for job in configs {
        println!("\n{}", style(format!("--- 处理任务: {} ---", job.job_name)).cyan().bold());
        println!("{} 仓库: {}", style("→").dim(), job.repo);
        println!("{} 目标: {}", style("→").dim(), job.target);
        if !job.restore_path.is_empty() {
             println!("{} 指定恢复子路径: {}", style("→").dim(), job.restore_path);
        }

        if !Path::new(&job.target).exists() {
            if let Err(e) = std::fs::create_dir_all(&job.target) {
                let err_msg = format!("创建目标目录 '{}' 失败: {}", job.target, e);
                summary.push(format!("{} {}: {}", style("✖").red(), job.job_name, err_msg));
                continue;
            }
        }

        let all_snapshots = match get_snapshots(restic_exe_path, &job.repo, &job.passwd) {
            Ok(snaps) => snaps,
            Err(e) => {
                let err_msg = format!("获取快照列表失败: {}", e);
                summary.push(format!("{} {}: {}", style("✖").red(), job.job_name, err_msg));
                continue;
            }
        };

        if all_snapshots.is_empty() {
            let err_msg = "仓库中未找到任何快照。".to_string();
            summary.push(format!("{} {}: {}", style("✖").red(), job.job_name, err_msg));
            continue;
        }

        let mut snapshots_to_restore: Vec<&Snapshot> = Vec::new();
        match job.snapshots.to_lowercase().trim() {
            "latest" => {
                if let Some(latest) = all_snapshots.first() {
                    snapshots_to_restore.push(latest);
                }
            },
            "all" => {
                snapshots_to_restore.extend(all_snapshots.iter());
            },
            ids_str => {
                let ids: Vec<&str> = ids_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                for id in ids {
                    if let Some(snap) = all_snapshots.iter().find(|s| s.short_id.starts_with(id)) {
                        snapshots_to_restore.push(snap);
                    } else {
                         let err_msg = format!("在仓库中未找到快照 ID: {}", id);
                         summary.push(format!("{} {}: {}", style("✖").red(), job.job_name, err_msg));
                    }
                }
            }
        }

        if snapshots_to_restore.is_empty() {
             let err_msg = "根据配置未找到匹配的快照进行恢复。".to_string();
             summary.push(format!("{} {}: {}", style("✖").red(), job.job_name, err_msg));
             continue;
        }

        let mut job_had_error = false;
        for snapshot in &snapshots_to_restore {
            println!("{} 正在恢复快照 {} 到 '{}'...", style("i").blue(), snapshot.short_id, job.target);
            
            let mut snapshot_arg = snapshot.short_id.clone();
            let mut error_in_snapshot = false;

            if !job.restore_path.is_empty() {
                let found_path = snapshot.paths.iter().find(|p| Path::new(p).ends_with(&job.restore_path));
                
                if let Some(full_path_str) = found_path {
                    let full_path = Path::new(full_path_str);
                    if let Some(parent) = full_path.parent() {
                        let restic_parent_path = utils::convert_to_restic_path(parent);
                        snapshot_arg = format!("{}:{}", snapshot.short_id, restic_parent_path);
                    }
                } else {
                    let err_msg = format!("在快照 {} 中未找到指定的子路径 '{}'", snapshot.short_id, job.restore_path);
                    summary.push(format!("{} {}: {}", style("✖").red(), job.job_name, err_msg));
                    job_had_error = true;
                    error_in_snapshot = true;
                }
            }
            
            if error_in_snapshot {
                break; 
            }

            let args = [
                "-r", &job.repo,
                "restore", &snapshot_arg,
                "--target", &job.target
            ];

            if let Err(e) = run_restic_command(restic_exe_path, &args, &job.passwd) {
                let err_msg = format!("恢复快照 {} 失败: {}", snapshot.short_id, e);
                summary.push(format!("{} {}: {}", style("✖").red(), job.job_name, err_msg));
                job_had_error = true;
                break;
            }
        }
        if !job_had_error {
            let success_msg = format!("成功恢复了 {} 个快照。", snapshots_to_restore.len());
            summary.push(format!("{} {}: {}", style("✔").green(), job.job_name, success_msg));
        }
    }

    println!("\n\n{}\n{}", style("===== 批量恢复汇总 =====").yellow().bold(), summary.join("\n"));
    Ok(())
}

fn get_snapshots(restic_exe_path: &str, repo_path: &str, password: &str) -> Result<Vec<Snapshot>, String> {
    let args = ["-r", repo_path, "snapshots", "--json"];
    let output = run_restic_command(restic_exe_path, &args, password)?;

    // 【修正】更新正则表达式以支持跨行匹配 (dotall flag `(?s)`)
    // Restic 输出的 JSON 可能是格式化过的，包含换行符，之前的 `.` 无法匹配换行
    let re = Regex::new(r"(?s)\[.*\]").unwrap();
    let json_str = match re.find(&output) {
        Some(m) => m.as_str(),
        None => {
             // 如果正则匹配失败，可以打印输出内容以帮助调试
             // eprintln!("DEBUG: Restic output did not contain a JSON array:\n{}", output);
             return Err("无法从 restic 输出中解析快照 JSON 数据。".to_string());
        }
    };

    let json_data: Value = serde_json::from_str(json_str)
        .map_err(|e| format!("解析快照 JSON 失败: {}. Raw JSON string: {}", e, json_str))?;

    let mut snapshots = Vec::new();
    if let Some(snaps_array) = json_data.as_array() {
        for snap in snaps_array {
            snapshots.push(Snapshot {
                short_id: snap["short_id"].as_str().unwrap_or("").to_string(),
                time: snap["time"].as_str().unwrap_or("").to_string(),
                paths: snap["paths"].as_array().map_or(vec![], |paths| {
                    paths.iter().map(|p| p.as_str().unwrap_or("").to_string()).collect()
                }),
                size: snap["size"].as_u64().unwrap_or(0),
            });
        }
    }
    snapshots.sort_by(|a, b| b.time.cmp(&a.time));
    Ok(snapshots)
}