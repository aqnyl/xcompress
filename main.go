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
	fmt.Println("xcompress 当前版本 v1.2.3")
	fmt.Println("👴 作者：菜玖玖emoji")
	fmt.Println("📺 bilibili：https://space.bilibili.com/395819372")
	fmt.Println("🧠 软件教程(失效记得艾特我)：https://www.yuque.com/xtnxnb/qo095a/tnve5f0rtnu9ad96?singleDoc#")
	fmt.Println("💰 本软件永久免费，亲爱的富哥大姐，如有能力可以点击下方链接请我一杯蜜雪冰城吗，谢谢啦！！！")
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

	// 添加汇总信息变量
	var backupSummary []string

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
		// fmt.Println("msg 格式: ", msg) // 这行可以注释掉或保留用于调试

		if configs, ok := msg.(map[string]res_func.TomlConfig); ok {
			for configName, config := range configs {
				fmt.Printf("\n正在处理配置项: %s (%s)\n", configName, config.Name)
				// 计算并显示最终备份路径
				finalRepoPath := filepath.Join(config.ResticHomePath, config.Name)
				fmt.Printf("最终备份仓库路径: %s\n", finalRepoPath)

				// 将路径添加到汇总中
				backupSummary = append(backupSummary, fmt.Sprintf("- %s -> %s", config.Name, finalRepoPath))

				// 处理合并备份逻辑
				if config.Merge == 1 {
					fmt.Printf("使用合并模式备份多个路径 (共%d个)\n", len(config.Path))
					// 创建固定名称的合并目录
					mergePath := filepath.Join(exeDir, config.MergeName)

					// 删除已存在的目录（如果存在）
					if err := os.RemoveAll(mergePath); err != nil {
						fmt.Printf("清理旧合并目录失败: %v\n", err)
						backupSummary = append(backupSummary, fmt.Sprintf("  - 合并备份失败 (清理旧目录错误)")) // 在汇总中标记错误
						continue                                                                   // 跳过此配置项
					}

					// 创建新目录
					if err := os.Mkdir(mergePath, 0755); err != nil {
						fmt.Printf("创建合并目录失败: %v\n", err)
						backupSummary = append(backupSummary, fmt.Sprintf("  - 合并备份失败 (创建合并目录错误)")) // 在汇总中标记错误
						continue                                                                    // 跳过此配置项
					}
					defer os.RemoveAll(mergePath) // 确保最终删除

					// 复制所有路径到合并目录的子目录
					copyError := false // 标记复制过程中是否有错误
					for _, srcPath := range config.Path {
						// 获取源路径的最后一级目录名
						baseName := filepath.Base(srcPath)
						targetPath := filepath.Join(mergePath, baseName)

						// 检查源是文件还是目录
						srcInfo, err := os.Stat(srcPath)
						if err != nil {
							fmt.Printf("获取源路径信息失败 %s: %v\n", srcPath, err)
							copyError = true
							break // 无法处理此源，跳出复制循环
						}

						var cmd *exec.Cmd
						if srcInfo.IsDir() {
							// 如果是目录，创建目标子目录并复制内容
							if err := os.MkdirAll(targetPath, 0755); err != nil {
								fmt.Printf("创建子目录失败: %v\n", err)
								copyError = true
								break
							}
							fmt.Printf("正在复制目录 %s 到 %s...\n", srcPath, targetPath)
							cmd = exec.Command("xcopy", srcPath, targetPath, "/E", "/I", "/H", "/Y") // 添加 /Y 覆盖已存在文件
						} else {
							// 如果是文件，直接复制到 mergePath 根目录
							fmt.Printf("正在复制文件 %s 到 %s...\n", srcPath, mergePath)
							cmd = exec.Command("xcopy", srcPath, mergePath, "/I", "/H", "/Y") // 复制文件不需要 /E
						}

						// 执行复制命令
						output, err := cmd.CombinedOutput()
						if err != nil {
							fmt.Printf("复制 %s 失败: %v\nOutput: %s\n", srcPath, err, string(output))
							copyError = true
							// 不再因为单个文件/目录复制失败而中断整个配置项，但标记错误
							// break
						} else {
							fmt.Printf("复制 %s 成功\n", srcPath)
						}
					}

					// 如果复制过程中出现错误，则跳过备份步骤
					if copyError {
						fmt.Println("由于文件/目录复制过程中出现错误，跳过此配置项的备份。")
						backupSummary = append(backupSummary, fmt.Sprintf("  - 合并备份失败 (文件复制错误)")) // 在汇总中标记错误
						continue                                                                  // 跳过此配置项的备份
					}

					// 执行合并备份
					fmt.Printf("开始合并备份 %s 到 %s...\n", mergePath, finalRepoPath)
					success, output := res_func.ResBackup(
						restic_exe_path,
						finalRepoPath, // 使用计算好的最终仓库路径
						mergePath,
						config.Passwd,
						128,
						"auto",
						config.Tag,
						true,
					)
					fmt.Println(output) // 打印 restic 的输出
					if !success {
						fmt.Println("合并备份失败")
						backupSummary = append(backupSummary, fmt.Sprintf("  - 合并备份失败")) // 在汇总中标记错误
					} else {
						fmt.Printf("合并备份成功: %s 已备份到 %s\n", mergePath, finalRepoPath)
						backupSummary = append(backupSummary, fmt.Sprintf("  - 合并备份成功")) // 在汇总中标记成功
						// 备份成功后清理空目录
						clearSuccess, clearOutput := res_func.ResClearFolder(finalRepoPath)
						fmt.Println("空目录清理结果:", clearOutput)
						if !clearSuccess {
							fmt.Println("警告：仓库清理未完全成功")
							backupSummary = append(backupSummary, fmt.Sprintf("    - 清理警告: %s", clearOutput)) // 在汇总中标记清理警告
						}
					}
				} else {
					// 单独备份每个路径
					pathSuccessCount := 0 // 记录成功的路径数
					for _, path := range config.Path {
						fmt.Printf("开始单独备份 %s 到 %s...\n", path, finalRepoPath)
						success, output := res_func.ResBackup(
							restic_exe_path,
							finalRepoPath, // 使用计算好的最终仓库路径
							path,
							config.Passwd,
							128,
							"auto",
							config.Tag,
							true,
						)
						fmt.Println(output) // 打印 restic 的输出
						if !success {
							fmt.Printf("路径 %s 备份失败\n", path)
							backupSummary = append(backupSummary, fmt.Sprintf("  - 路径 %s 备份失败", path)) // 在汇总中标记错误
						} else {
							pathSuccessCount++
							fmt.Printf("备份成功: %s 已备份到 %s\n", path, finalRepoPath)
							// 备份成功后清理空目录
							clearSuccess, clearOutput := res_func.ResClearFolder(finalRepoPath)
							fmt.Println("空目录清理结果:", clearOutput)
							if !clearSuccess {
								fmt.Println("警告：仓库清理未完全成功")
								backupSummary = append(backupSummary, fmt.Sprintf("    - 清理警告 (%s): %s", path, clearOutput)) // 在汇总中标记清理警告
							}
						}
					}
					// 根据成功备份的路径数更新汇总信息
					if pathSuccessCount == len(config.Path) {
						backupSummary = append(backupSummary, fmt.Sprintf("  - 所有 %d 个路径单独备份成功", len(config.Path)))
					} else {
						backupSummary = append(backupSummary, fmt.Sprintf("  - %d/%d 个路径单独备份成功", pathSuccessCount, len(config.Path)))
					}
				}
			}

			// 显示备份汇总信息
			if len(backupSummary) > 0 {
				fmt.Println("\n===== 备份汇总 =====")
				for _, summary := range backupSummary {
					fmt.Println(summary)
				}
				fmt.Println("=====================")
			} else {
				fmt.Println("\n没有有效的配置项被处理。")
			}
		} else {
			// 如果 msg 不是预期的 map 类型，打印错误
			fmt.Println("\n错误：配置文件解析成功，但返回的数据格式不正确。")
		}
	} else {
		// 当 Toml_parse 返回 false 时，打印错误信息
		if errMsg, ok := msg.(string); ok {
			fmt.Println("\n错误：配置文件解析或验证失败:")
			fmt.Println(errMsg) // 打印 Toml_parse 返回的具体错误信息
		} else {
			// 处理其他可能的错误情况
			fmt.Println("\n错误：配置文件解析时发生未知错误")
		}

		// 检查是否是因配置文件不存在且无参数而进入交互模式
		if len(os.Args) == 1 && !pathExists(toml_path) {
			fmt.Println("\n未找到默认配置文件 backup_config.toml，进入交互模式...")
			var backupPath string
			fmt.Print("请输入要备份的路径: ")
			// 使用 bufio 读取可能包含空格的路径
			reader := bufio.NewReader(os.Stdin)
			backupPath, _ = reader.ReadString('\n')
			backupPath = strings.TrimSpace(backupPath)

			if backupPath == "" {
				fmt.Println("错误：输入的路径不能为空。")
			} else {
				// 检查输入的路径是否存在
				if !pathExists(backupPath) {
					fmt.Printf("错误：输入的路径 '%s' 不存在。\n", backupPath)
				} else {
					success, resultMsg := InteractionBackup(restic_exe_path, backupPath, exeDir) // 修改变量名避免冲突
					if success {
						fmt.Println("\n交互模式备份结果:", resultMsg)
					} else {
						fmt.Println("\n错误：交互模式备份失败，错误信息：") // 调整错误提示
						fmt.Println(resultMsg)
					}
				}
			}
		} else if len(os.Args) > 1 && !strings.HasSuffix(os.Args[1], ".toml") && !pathExists(os.Args[1]) {
			// 处理命令行提供了无效路径参数的情况
			fmt.Printf("\n错误：命令行提供的路径参数 '%s' 不存在。\n", os.Args[1])
		}
	}

	// 确保最后的等待输入始终执行
	fmt.Print("\n操作完成，按任意键退出...") // 修改提示信息
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
