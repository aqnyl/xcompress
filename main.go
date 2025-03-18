package main

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"xcompress_cmd/res_func"
)

func main() {
	exePath, err := os.Executable()
	if err != nil {
		// 处理可执行文件路径获取错误
		exePath = "." // 失败时使用当前目录
	}

	// 优先使用当前工作目录
	currentDir, err := os.Getwd()
	if err != nil {
		currentDir = filepath.Dir(exePath) // 回退到可执行文件目录
	}
	toml_path := filepath.Join(currentDir, "backup_config.toml")
	fmt.Println("toml_path: ", toml_path)
	var result bool
	var msg interface{}
	result, msg = res_func.Toml_parse(toml_path)
	if result {
		// example
		// 数据类型：map[string]res_func.TomlConfig
		// map[one:{one_res [D:\work_folder\tools\automatic\backup_file\test_dir\test_to_backup\one_folder] test 123456 D:\work_folder\tools\automatic\backup_file\test_dir\restic_backup_dir} two:{two_res [D:\work_folder\tools\automatic\backup_file\test_dir\test_to_backup\two_folder\two D:\work_folder\tools\automatic\backup_file\test_dir\test_to_backup\two_folder\one] test 123456 D:\work_folder\tools\automatic\backup_file\test_dir\restic_backup_dir}]
		fmt.Println("toml 文件解析成功")
		if configs, ok := msg.(map[string]res_func.TomlConfig); ok {
			for _, config := range configs {
				fmt.Printf("\n正在处理配置项: %s\n", config.Name)

				// 处理合并备份逻辑
				if config.Merge == 1 {
					// 创建临时目录
					tempDir, err := os.MkdirTemp(currentDir, "temporary_")
					if err != nil {
						fmt.Printf("创建临时目录失败: %v\n", err)
						continue
					}
					defer os.RemoveAll(tempDir)

					// 复制所有路径到临时目录
					for _, srcPath := range config.Path {
						cmd := exec.Command("xcopy", srcPath, tempDir, "/E", "/I", "/H")
						if err := cmd.Run(); err != nil {
							fmt.Printf("复制文件失败: %v\n", err)
							continue
						}
					}

					// 执行合并备份
					success, output := res_func.ResBackup(
						filepath.Join(config.ResticHomePath, config.Name),
						tempDir,
						config.Passwd,
						128,
						"auto",
						config.Tag,
						true,
					)
					fmt.Println(output)
					if !success {
						fmt.Println("合并备份失败")
					}
				} else {
					// 单独备份每个路径
					for _, path := range config.Path {
						success, output := res_func.ResBackup(
							filepath.Join(config.ResticHomePath, config.Name),
							path,
							config.Passwd,
							128,
							"auto",
							config.Tag,
							true,
						)
						fmt.Println(output)
						if !success {
							fmt.Printf("路径 %s 备份失败\n", path)
						}
					}
				}
			}
		}
	} else {
		fmt.Println("toml 文件解析失败")
		fmt.Println(msg)
		fmt.Println("按任意键退出程序...")
		fmt.Scanln() // 等待用户输入
		return
	}

	fmt.Print("备份操作完成，按任意键退出...")
	fmt.Scanln() // 等待用户输入
}
