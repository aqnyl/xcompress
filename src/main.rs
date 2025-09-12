mod utils;
mod config;
mod backup;
mod restore;
mod help;

use std::env;
use console::style;
use dialoguer::{theme::ColorfulTheme, Select};

fn main() {
    // 首次运行时先打印一次
    let _ = console::Term::stdout().clear_screen();
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
    let items = &[
        "备份 (Compress)", 
        "恢复 (Decompress)", 
        "批量备份 (Batch Backup)", 
        "批量恢复 (Batch Restore)",
        "查看帮助 (View Help)",
        "退出 (Exit)"
    ];
    let theme = ColorfulTheme::default();

    loop {
        let selection = Select::with_theme(&theme)
            .with_prompt("请选择要执行的操作")
            .items(items)
            .default(0)
            .interact_opt()
            .unwrap();

        let mut should_exit_loop = false;

        match selection {
            Some(0) => { // 备份
                backup::handle_backup(restic_exe_path, None, None);
                should_exit_loop = true;
            }
            Some(1) => { // 恢复
                if let Err(e) = restore::handle_restore(restic_exe_path) {
                    eprintln!("\n{} {}", style("✖ 恢复操作失败:").red().bold(), style(e).red());
                }
                should_exit_loop = true;
            }
            Some(2) => { // 批量备份
                if let Err(e) = backup::handle_batch_backup(restic_exe_path) {
                    eprintln!("\n{} {}", style("✖ 批量备份操作失败:").red().bold(), style(e).red());
                }
                should_exit_loop = true;
            }
            Some(3) => { // 批量恢复
                if let Err(e) = restore::handle_batch_restore(restic_exe_path) {
                    eprintln!("\n{} {}", style("✖ 批量恢复操作失败:").red().bold(), style(e).red());
                }
                should_exit_loop = true;
            }
            Some(4) => { // 查看帮助
                let _ = console::Term::stdout().clear_screen();
                help::print_help_info();
                let _ = console::Term::stdout().clear_screen();
                utils::print_header();
                // 不退出循环，返回主菜单
            }
            Some(5) | None => { // 退出
                println!("\n{}", style("👋 程序已退出，感谢使用！").yellow());
                return; // 直接退出函数
            }
            _ => unreachable!(),
        }

        if should_exit_loop {
            break;
        }
    }
}


fn wait_for_exit() {
    println!("\n\n{}", style("操作完成，按 Enter 键退出...").dim());
    let mut buffer = String::new();
    std::io::stdin().read_line(&mut buffer).unwrap();
}