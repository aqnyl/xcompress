mod utils;
mod config;
mod backup;
mod restore;

use std::env;
use console::style;
use dialoguer::{theme::ColorfulTheme, Select};

fn main() {
    utils::print_header();

    // 1. 检查 Restic 环境
    let restic_exe_path = match utils::check_restic_path() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("{}", e);
            wait_for_exit();
            return;
        }
    };

    // 2. 解析命令行参数
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        // 如果有参数，直接处理并退出，不显示菜单
        let first_arg = &args[1];
        if first_arg.ends_with(".toml") {
            backup::handle_backup(&restic_exe_path, Some(first_arg.clone()), None);
        } else {
            backup::handle_backup(&restic_exe_path, None, Some(first_arg.clone()));
        }
    } else {
        // 如果没有参数，显示交互式主菜单
        show_main_menu(&restic_exe_path);
    }
    
    wait_for_exit();
}

fn show_main_menu(restic_exe_path: &str) {
    let items = &["备份 (Compress)", "恢复 (Decompress)", "退出 (Exit)"];
    let theme = ColorfulTheme::default();

    loop {
        let selection = Select::with_theme(&theme)
            .with_prompt("请选择要执行的操作")
            .items(items)
            .default(0)
            .interact_opt()
            .unwrap();

        match selection {
            Some(0) => {
                backup::handle_backup(restic_exe_path, None, None);
                break;
            }
            Some(1) => {
                if let Err(e) = restore::handle_restore(restic_exe_path) {
                    eprintln!("\n{} {}", style("✖ 恢复操作失败:").red().bold(), style(e).red());
                }
                break;
            }
            Some(2) | None => { // None 对应用户按 Esc 或 Ctrl+C
                println!("\n{}", style("👋 程序已退出，感谢使用！").yellow());
                return;
            }
            _ => unreachable!(),
        }
    }
}

fn wait_for_exit() {
    println!("\n\n{}", style("操作完成，按 Enter 键退出...").dim());
    let mut buffer = String::new();
    std::io::stdin().read_line(&mut buffer).unwrap();
}