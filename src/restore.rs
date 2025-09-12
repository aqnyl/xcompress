use crate::utils::{self, run_restic_command};
use console::style;
use dialoguer::{theme::ColorfulTheme, Input, Password, Select};
use serde_json::Value;
use std::env;
use std::path::Path;
use regex::Regex;

struct Snapshot {
    short_id: String,
    time: String,
    paths: Vec<String>,
}

pub fn handle_restore(restic_exe_path: &str) -> Result<(), String> {
    println!("\n{}\n", style("--- 开始恢复流程 ---").bold().yellow());
    
    let theme = ColorfulTheme::default();
    
    // 获取仓库路径
    let repo_path_str: String = Input::with_theme(&theme)
        .with_prompt("请输入或拖入 restic 仓库路径")
        .interact_text()
        .map_err(|e| e.to_string())?;
    
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

    // 让用户选择快照
    let snapshot_items: Vec<String> = snapshots
        .iter()
        .map(|s| format!("{}  ({})  {}", s.short_id, s.time.split('T').next().unwrap_or(""), s.paths.join(", ")))
        .collect();
    
    let selection = Select::with_theme(&theme)
        .with_prompt("请选择要恢复的快照 (按 'q' 退出)")
        .items(&snapshot_items)
        .default(0)
        .interact_opt()
        .map_err(|e| e.to_string())?;

    let selected_snapshot = match selection {
        Some(index) => &snapshots[index],
        None => {
            println!("{}", style("操作已取消。").yellow());
            return Ok(());
        }
    };
    
    // 决定输出路径
    let current_dir = env::current_dir().map_err(|e| e.to_string())?;
    let default_output_path = current_dir.to_string_lossy();
    let output_path_str: String = Input::with_theme(&theme)
        .with_prompt("请输入恢复目标路径 (留空则恢复到当前目录)")
        .default(default_output_path.to_string())
        .interact_text()
        .map_err(|e| e.to_string())?;
    
    println!("\n{} 準備恢復快照 {} 到 '{}'...", style("i").blue(), selected_snapshot.short_id, output_path_str);
    
    // 执行恢复命令
    let args = [
        "-r", &repo_path.to_string_lossy(),
        "restore", &selected_snapshot.short_id,
        "--target", &output_path_str
    ];

    match run_restic_command(restic_exe_path, &args, &password) {
        Ok(output) => {
            println!("{}\n{}", style("✔ 恢复成功!").green().bold(), output);
            Ok(())
        },
        Err(e) => Err(format!("恢复失败: {}", e)),
    }
}

fn get_snapshots(restic_exe_path: &str, repo_path: &str, password: &str) -> Result<Vec<Snapshot>, String> {
    let args = ["-r", repo_path, "snapshots", "--json"];
    let output = run_restic_command(restic_exe_path, &args, password)?;

    // restic 的 json 输出可能不是严格的 json 数组，需要正则提取
    let re = Regex::new(r"\[.*\]").unwrap();
    let json_str = match re.find(&output) {
        Some(m) => m.as_str(),
        None => return Err("无法从 restic 输出中解析快照 JSON 数据。".to_string()),
    };

    let json_data: Value = serde_json::from_str(json_str)
        .map_err(|e| format!("解析快照 JSON 失败: {}", e))?;

    let mut snapshots = Vec::new();
    if let Some(snaps_array) = json_data.as_array() {
        for snap in snaps_array {
            snapshots.push(Snapshot {
                short_id: snap["short_id"].as_str().unwrap_or("").to_string(),
                time: snap["time"].as_str().unwrap_or("").to_string(),
                paths: snap["paths"].as_array().map_or(vec![], |paths| {
                    paths.iter().map(|p| p.as_str().unwrap_or("").to_string()).collect()
                }),
            });
        }
    }
    snapshots.sort_by(|a, b| b.time.cmp(&a.time)); // 按时间倒序
    Ok(snapshots)
}