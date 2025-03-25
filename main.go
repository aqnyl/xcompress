package main

import (
	"bufio"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"xcompress_cmd/res_func"
)

func main() {
	// 检查电脑是否有 restic 软件
	fmt.Println("xcompress 当前版本 v1.2.2")
	fmt.Println("👴 作者：菜玖玖emoji")
	fmt.Println("📺 bilibili：https://space.bilibili.com/395819372")
	fmt.Println("🧠 软件教程(失效记得艾特我)：https://www.yuque.com/xtnxnb/qo095a/tnve5f0rtnu9ad96?singleDoc#")
	fmt.Println("💰 本软件永久免费，亲爱的富哥大姐，如有能力可以点击下方链接请我一杯米雪冰城吗，谢谢啦！！！")
	fmt.Println("💰 https://afdian.com/a/wocaijiujiu")
	fmt.Println("👏 感谢使用 ヾ(≧▽≦*)o\n")

	var toml_path string
	var restic_exe_path string
	exePath, err := os.Executable() // 获取可执行文件路径
	exeDir := filepath.Dir(exePath) // 获取可执行文件所在目录
	if err != nil {
		fmt.Println("获取可执行文件路径失败:", err)
		fmt.Println("请将xcompress.exe放在工作目录下运行")
		fmt.Println("按任意键退出...")
		bufio.NewReader(os.Stdin).ReadBytes('\n')
		os.Exit(1)
	}
	resticPath_result := res_func.CheckResticPath()
	switch resticPath_result {
	case 0:
		fmt.Println("未找到可用restic")
		os.Exit(1)
	case 1:
		fmt.Println("系统PATH中找到restic，使用系统restic")
		restic_exe_path = "restic"
	case 2:
		fmt.Println("当前程序目录找到restic.exe，使用当前程序restic")
		restic_exe_path = filepath.Join(exeDir, "restic.exe")
	}

	// 解析程序所传入的命令行参数，如果为toml文件，则使用该文件作为toml文件路径
	// 如果为路径，则直接备份该路径
	if len(os.Args) > 1 {
		// 启动程序传入层数，则使用传入的层数
		argPath := os.Args[1]
		if strings.HasSuffix(argPath, ".toml") {
			toml_path = argPath
		} else {
			// 检查路径是否存在
			if pathExists(argPath) {
				success, result := InteractionBackup(restic_exe_path, argPath, exeDir)
				switch success {
				case true:
					fmt.Println("\n备份结果:", result)
				case false:
					fmt.Println("错误：备份失败，错误信息：")
					fmt.Println(result)
				}
			} else {
				fmt.Println("错误：指定的路径不存在")
			}
			// 添加退出等待
			fmt.Print("按任意键退出...")
			bufio.NewReader(os.Stdin).ReadBytes('\n')
			return // 提前退出主函数
		}
	} else {
		// 启动程序没有传入层数，则使用默认路径
		currentDir, _ := os.Getwd()
		toml_path = filepath.Join(currentDir, "backup_config.toml")
	}
	fmt.Println("toml_path: ", toml_path)
	var result bool
	var msg interface{}
	result, msg = res_func.Toml_parse(toml_path)
	// 返回字典格式说明:
	// 键: 配置项名称（config文件中的键名）
	// 值: TomlConfig 结构体，包含以下字段:
	// Name: 配置名称 (string)
	// Path: 备份路径列表 ([]string)
	// Tag: 备份标签 (string)
	// Passwd: 仓库密码 (string)
	// ResticHomePath: 仓库路径 (string)
	// Merge: 路径合并模式 0-不合并 1-合并 (int)
	// MergeName: 合并后的路径名称 (string)
	if result {
		// example
		// 数据类型：map[string]res_func.TomlConfig
		// map[one:{one_res [D:\work_folder\tools\automatic\backup_file\test_dir\test_to_backup\one_folder] test 123456 D:\work_folder\tools\automatic\backup_file\test_dir\restic_backup_dir} two:{two_res [D:\work_folder\tools\automatic\backup_file\test_dir\test_to_backup\two_folder\two D:\work_folder\tools\automatic\backup_file\test_dir\test_to_backup\two_folder\one] test 123456 D:\work_folder\tools\automatic\backup_file\test_dir\restic_backup_dir}]
		fmt.Println("toml 文件解析成功")
		fmt.Println("msg 格式: ", msg)

		if configs, ok := msg.(map[string]res_func.TomlConfig); ok {
			for _, config := range configs {
				fmt.Printf("\n正在处理配置项: %s\n", config.Name)

				// 处理合并备份逻辑
				if config.Merge == 1 {
					// 创建固定名称的合并目录
					mergePath := filepath.Join(exeDir, config.MergeName)

					// 删除已存在的目录（如果存在）
					if err := os.RemoveAll(mergePath); err != nil {
						fmt.Printf("清理旧目录失败: %v\n", err)
						continue
					}

					// 创建新目录
					if err := os.Mkdir(mergePath, 0755); err != nil {
						fmt.Printf("创建合并目录失败: %v\n", err)
						continue
					}
					defer os.RemoveAll(mergePath) // 确保最终删除

					// 复制所有路径到合并目录的子目录
					for _, srcPath := range config.Path {
						// 获取源路径的最后一级目录名
						baseName := filepath.Base(srcPath)
						targetPath := filepath.Join(mergePath, baseName)

						// 创建目标子目录
						if err := os.MkdirAll(targetPath, 0755); err != nil {
							fmt.Printf("创建子目录失败: %v\n", err)
							continue
						}

						// 复制到子目录
						cmd := exec.Command("xcopy", srcPath, targetPath, "/E", "/I", "/H")
						if err := cmd.Run(); err != nil {
							fmt.Printf("复制文件到 %s 失败: %v\n", baseName, err)
							continue
						}
					}

					// 执行合并备份
					success, output := res_func.ResBackup(
						restic_exe_path,
						filepath.Join(config.ResticHomePath, config.Name),
						mergePath,
						config.Passwd,
						128,
						"auto",
						config.Tag,
						true,
					)
					res_func.ResClearFolder(filepath.Join(config.ResticHomePath, config.Name)) // 清理空目录
					fmt.Println(output)
					if !success {
						fmt.Println("合并备份失败")
					} else {
						// 备份成功后清理空目录
						clearSuccess, clearOutput := res_func.ResClearFolder(
							filepath.Join(config.ResticHomePath, config.Name),
						)
						fmt.Println("空目录清理结果:", clearOutput)
						if !clearSuccess {
							fmt.Println("警告：仓库清理未完全成功")
						}
					}
				} else {
					// 单独备份每个路径
					for _, path := range config.Path {
						success, output := res_func.ResBackup(
							restic_exe_path,
							filepath.Join(config.ResticHomePath, config.Name),
							path,
							config.Passwd,
							128,
							"auto",
							config.Tag,
							true,
						)
						res_func.ResClearFolder(filepath.Join(config.ResticHomePath, config.Name)) // 清理空目录
						fmt.Println(output)
						if !success {
							fmt.Printf("路径 %s 备份失败\n", path)
						} else {
							// 备份成功后清理空目录
							clearSuccess, clearOutput := res_func.ResClearFolder(
								filepath.Join(config.ResticHomePath, config.Name),
							)
							fmt.Println("空目录清理结果:", clearOutput)
							if !clearSuccess {
								fmt.Println("警告：仓库清理未完全成功")
							}
						}
					}
				}
			}
		}
	} else {
		// 当没有toml文件且无参数时进入交互模式
		if len(os.Args) == 1 && !pathExists(toml_path) {
			fmt.Println("未找到配置文件，进入交互模式")
			var backupPath string
			fmt.Print("请输入要备份的路径: ")
			fmt.Scanln(&backupPath)
			success, result := InteractionBackup(restic_exe_path, backupPath, exeDir)
			if success {
				fmt.Println("\n备份结果:", result)
			} else {
				fmt.Println("错误：指定的路径不存在")
			}
			// 添加退出等待
			fmt.Print("按任意键退出...")
			bufio.NewReader(os.Stdin).ReadBytes('\n')
			return // 提前退出主函数
		}
	}

	// 确保最后的等待输入始终执行
	fmt.Print("备份操作完成，按任意键退出...")
	bufio.NewReader(os.Stdin).ReadBytes('\n')
}

// InteractionBackup 处理交互式备份流程
// 参数:
//
//	restic_exe_path string - restic 可执行文件路径
//	backupPath string - 需要备份的原始路径
//	exeDir string - 可执行文件所在目录
//
// 返回值:
//
//	bool - 备份是否成功
//	string - 备份结果描述信息
//
// 功能:
//  1. 验证备份路径有效性
//  2. 通过命令行交互获取备份名称和密码
//  3. 自动生成仓库路径
//  4. 调用底层备份函数执行实际备份操作
func InteractionBackup(restic_exe_path, backupPath string, exeDir string) (bool, string) {
	// 验证路径存在性
	if !pathExists(backupPath) {
		return false, "路径不存在"
	}
	fmt.Println("backupPath: ", backupPath)

	reader := bufio.NewReader(os.Stdin)

	// 获取备份名称
	fmt.Print("请输入备份名称: ")
	backupName, _ := reader.ReadString('\n')   // 读取输入
	backupName = strings.TrimSpace(backupName) // 去除空格

	// 获取密码
	fmt.Print("请输入备份密码: ")
	passwd, _ := reader.ReadString('\n')
	passwd = strings.TrimSpace(passwd)

	// 修改仓库路径生成逻辑
	// 直接使用可执行文件所在目录（不再取父目录）
	resticPath := filepath.Join(exeDir, backupName)
	fmt.Println("resticPath: ", resticPath)

	fmt.Printf("\n正在备份 %s 到 %s...\n", backupPath, resticPath)

	// 调用备份函数时移除直接退出逻辑
	success, result := res_func.ResBackup(
		restic_exe_path,
		backupName,
		backupPath,
		passwd,
		128,    // 固定packSize
		"auto", // 固定压缩模式
		"",     // 无tag
		false,  // 不跳过未修改文件
	)
	res_func.ResClearFolder(backupName) // 清理空目录

	// 返回结果但不退出程序
	return success, result
}

// pathExists 检查指定路径是否存在
// 参数:
//
//	path string - 需要检查的文件/目录路径
//
// 返回值:
//
//	bool - 路径存在返回true，否则返回false
func pathExists(path string) bool {
	_, err := os.Stat(path)
	return !os.IsNotExist(err)
}
