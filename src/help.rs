use console::style;
use std::io;

pub fn print_help_info() {
    let header = |s| style(s).yellow().bold();
    let cmd = |s| style(s).cyan();
    let path = |s| style(s).dim();
    let sect = |s| style(s).magenta().bold().underlined();
    let opt = |s| style(s).green();

    println!("\n{}", header("======================= xcompress 帮助中心 ======================="));

    println!("\n{}", sect("程序简介"));
    println!("xcompress 是一个基于 restic 的图形化备份/恢复助手，旨在简化 restic 的使用流程。");
    println!("支持通过配置文件进行批量操作，也支持简单易懂的交互式操作。");

    println!("\n{}", sect("使用模式"));
    println!("程序主要有三种启动方式：");

    println!("\n  {}", header("1. 命令行模式 (直接提供参数)"));
    println!("    - 直接备份单个文件或目录:");
    println!("      {} xcompress.exe {}", cmd("  >"), path("D:\\MyProject"));
    println!("      程序将进入交互模式，引导您为该目录创建备份。");
    println!("\n    - 使用配置文件进行批量备份:");
    println!("      {} xcompress.exe {}", cmd("  >"), path("my_backup_jobs.toml"));
    println!("      程序将自动读取指定的 toml 文件并执行所有备份任务。");

    println!("\n  {}", header("2. 交互式菜单模式 (无参数启动)"));
    println!("    直接运行 {} 将进入主菜单，提供以下选项：", cmd("xcompress.exe"));
    println!("    - {}：通过交互式问答备份单个文件/目录。程序会扫描并让您选择仓库。", opt("备份 (Compress)"));
    println!("    - {}：通过交互式问答恢复一个仓库中的快照。", opt("恢复 (Decompress)"));
    println!("    - {}：选择一个 backup_config.toml 文件进行批量备份。", opt("批量备份 (Batch Backup)"));
    println!("    - {}：选择一个 restore_config.toml 文件进行批量恢复。", opt("批量恢复 (Batch Restore)"));
    println!("    - {}：显示当前帮助信息。", opt("查看帮助 (View Help)"));
    println!("    - {}：退出程序。", opt("退出 (Exit)"));
    println!("\n    {}", style("注意：如果在程序目录下存在 backup_config.toml 文件，无参数启动时会优先执行批量备份，而不是显示主菜单。").dim());

    println!("\n{}", sect("配置文件详解"));

    println!("\n  {}", header("备份配置 (例如 backup_config.toml)"));
    println!(r#"
    # 全局配置 (可选)
    [global_config]
    passwd = "global_password"
    restic_home_path = "D:\\all_my_restic_repos" # 所有仓库的存放根目录
    tag = "daily"
    # pack_size: 128 (推荐) = 文件数少、压缩率最高、适合网盘；16 = 碎片多、本地性能最高。
    pack_size = 128 

    # 备份任务配置 (可以有多个)
    [config.project_A] # "project_A" 是任务的唯一标识
    name = "Project_A_Backup" # 仓库目录名，会拼接在 restic_home_path 后面
    path = ["D:\\code\\projectA", "C:\\docs\\projectA_docs"] # 需要备份的路径列表
    # merge = 1 # 设置为1时，会先将所有 path 的内容复制到临时目录再整体备份
    # merge_name = "merged_projA" # merge=1 时的临时目录名前缀

    [config.photos]
    name = "My_Photos"
    path = ["E:\\Photos"]
    passwd = "photo_password_123" # 单独为此任务设置密码
    "#);

    println!("\n  {}", header("恢复配置 (例如 restore_config.toml)"));
    println!(r#"
    # 全局配置 (可选)
    [global]
    passwd = "default_password"

    # 恢复任务配置 (可以有多个)
    [restore_jobs.restore_projA]
    repo = "D:\\all_my_restic_repos\\Project_A_Backup" # 【必填】要从哪个仓库恢复
    target = "D:\\restored_files\\project_A"      # 【必填】恢复到哪里
    # 【可选】要恢复的快照ID。可选值为:
    # "latest" (默认), "all", 或 "id1,id2,id3" (短ID列表)
    snapshots = "latest"
    # 【可选】指定只恢复快照中的某个特定子路径，并剥离其上层目录。
    # 这对于从 D:\work\proj\my_app 的备份中只恢复出 my_app 目录非常有用。
    # 填写的值应该是原始备份路径的一部分，例如 "my_app" 或 "proj\\my_app"
    restore_path = "projectA_docs" 

    [restore_jobs.restore_photos_by_id]
    repo = "D:\\all_my_restic_repos\\My_Photos"
    target = "D:\\restored_files\\photos"
    snapshots = "a1b2c3d4, e5f6g7h8"
    # 如果不指定 restore_path，则会按 restic 默认行为恢复整个快照（带完整路径）
    "#);

    println!("\n{}", header("==================================================================="));
    println!("\n{}", style("按 Enter 键返回主菜单...").dim());
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer).unwrap();
}