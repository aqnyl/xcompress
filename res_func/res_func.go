package res_func

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"

	"github.com/BurntSushi/toml"
)

// 配置项结构体
type TomlConfig struct {
	Name           string   `toml:"name"`
	Path           []string `toml:"path"`
	Tag            string   `toml:"tag"`
	Passwd         string   `toml:"passwd"`
	ResticHomePath string   `toml:"restic_home_path"`
	Merge          int      `toml:"merge"`
	MergeName      string   `toml:"merge_name"`
}

// 添加全局配置结构体
type GlobalConfig struct {
	Merge          int    `toml:"merge"`
	Passwd         string `toml:"passwd"`
	ResticHomePath string `toml:"restic_home_path"`
	Tag            string `toml:"tag"`
}

// 修改顶层配置结构
type TomlConfigFile struct {
	Global  GlobalConfig          `toml:"global_config"`
	Configs map[string]TomlConfig `toml:"config"`
}

// Toml_parse TOML配置文件解析与验证函数
// 参数:
//
//	filePath - TOML配置文件的完整路径
//
// 返回值:
//
//	bool - 验证是否通过(true表示验证成功)
//	interface{} - 成功时返回 map[string]TomlConfig 字典，失败时返回错误信息字符串
//	            返回字典格式说明:
//	            键: 配置项名称（config文件中的键名）
//	            值: TomlConfig 结构体，包含以下字段:
//	                Name: 配置名称 (string)
//	                Path: 备份路径列表 ([]string)
//	                Tag: 备份标签 (string)
//	                Passwd: 仓库密码 (string)
//	                ResticHomePath: 仓库路径 (string)
//	                Merge: 路径合并模式 0-不合并 1-合并 (int)
//	                MergeName: 合并后的路径名称 (string)
func Toml_parse(filePath string) (bool, interface{}) {
	var configFile TomlConfigFile
	var errorMessages string

	// 解析TOML文件
	if _, err := toml.DecodeFile(filePath, &configFile); err != nil {
		return false, fmt.Sprintf("TOML解析错误: %v", err)
	}

	// 创建最终配置集合
	finalConfigs := make(map[string]TomlConfig)

	// 遍历所有配置项
	for configName, cfg := range configFile.Configs {
		// 合并配置（局部配置优先）
		finalCfg := TomlConfig{
			Name:           cfg.Name,
			Path:           cfg.Path,
			Tag:            cfg.Tag,
			Passwd:         cfg.Passwd,
			ResticHomePath: cfg.ResticHomePath,
			Merge:          cfg.Merge,
			MergeName:      cfg.MergeName,
		}

		// 合并全局配置（修改判断逻辑）
		if finalCfg.Passwd == "" {
			finalCfg.Passwd = configFile.Global.Passwd
		}
		if finalCfg.ResticHomePath == "" {
			finalCfg.ResticHomePath = configFile.Global.ResticHomePath
		}
		if finalCfg.Tag == "" {
			finalCfg.Tag = configFile.Global.Tag
		}

		// 修改merge字段合并逻辑（新增智能默认值）
		if finalCfg.Merge == 0 { // 配置项未设置时使用全局配置
			finalCfg.Merge = configFile.Global.Merge
		}
		// 当全局和配置项都未设置时，根据path数量设置默认值
		if finalCfg.Merge == 0 {
			if len(finalCfg.Path) > 1 {
				finalCfg.Merge = 1
			} else {
				finalCfg.Merge = 0
			}
		}

		// 设置 merge_name 默认值
		if finalCfg.MergeName == "" {
			finalCfg.MergeName = "merge_path"
		}

		// 验证必填字段
		if finalCfg.Name == "" {
			errorMessages += fmt.Sprintf("配置项[%s]: name字段缺失\n", configName)
		}
		if len(finalCfg.Path) == 0 {
			errorMessages += fmt.Sprintf("配置项[%s]: path字段缺失\n", configName)
		}
		if finalCfg.Passwd == "" {
			errorMessages += fmt.Sprintf("配置项[%s]: passwd字段缺失（全局配置也未设置）\n", configName)
		}
		if finalCfg.ResticHomePath == "" {
			errorMessages += fmt.Sprintf("配置项[%s]: restic_home_path字段缺失（全局配置也未设置）\n", configName)
		}

		// 验证路径存在性
		for _, p := range finalCfg.Path {
			if !pathExists(p) {
				errorMessages += fmt.Sprintf("配置项[%s]: 路径不存在 - %s\n", configName, p)
			}
		}
		fmt.Println("finalCfg.ResticHomePath: ----------", finalCfg.ResticHomePath)
		if !pathExists(finalCfg.ResticHomePath) {
			errorMessages += fmt.Sprintf("配置项[%s]: restic仓库路径不存在 - %s\n", configName, finalCfg.ResticHomePath)
		}

		// 新增 merge 字段验证
		if finalCfg.Merge != 0 && finalCfg.Merge != 1 {
			errorMessages += fmt.Sprintf("配置项[%s]: merge字段必须为0或1\n", configName)
		}
		fmt.Println("finalCfg: ----------", finalCfg)

		// 将合并后的配置存入最终集合
		finalConfigs[configName] = finalCfg
	}

	if errorMessages != "" {
		return false, errorMessages
	}
	// 返回合并后的最终配置
	fmt.Println("Final configs: ----------", finalConfigs)
	return true, finalConfigs
}

// pathExists 路径存在性检查函数
// 参数:
//
//	path - 需要检查的路径字符串
//
// 返回值:
//
//	bool - 路径是否存在(true表示路径存在)
func pathExists(path string) bool {
	if path == "" {
		return false
	}
	if _, err := os.Stat(path); os.IsNotExist(err) {
		return false
	}
	return true
}

// ResticInit 初始化 restic 仓库
// 参数:
//   - restic_exe_path: restic 可执行文件路径
//   - resticPath: restic 仓库路径
//   - passwd: 仓库加密密码
//
// 返回值:
//   - bool: 操作是否成功
//   - string: 成功时返回输出信息，失败时返回错误信息
func ResticInit(restic_exe_path, resticPath, passwd string) (bool, string) {
	cmd := exec.Command(restic_exe_path, "init", "-r", resticPath, "--repository-version", "2")
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return false, fmt.Sprintf("Error creating stdin pipe: %v", err)
	}

	go func() {
		defer stdin.Close()
		io.WriteString(stdin, passwd+"\n")
	}()

	output, err := cmd.CombinedOutput()
	if err != nil {
		return false, fmt.Sprintf("Error: %v, Output: %s", err, output)
	}

	if cmd.ProcessState.ExitCode() == 0 {
		return true, string(output)
	}
	return false, string(output)
}

// ResticBackup 执行备份操作
// 参数:
//   - restic_exe_path: restic 可执行文件路径
//   - resticPath: restic 仓库路径
//   - backupPath: 需要备份的路径
//   - passwd: 仓库加密密码
//   - packSize: 分块大小(MB)，有效范围 1-128
//   - compression: 压缩模式（auto/off/max）
//   - tag: 备份标签
//   - skip: 是否跳过未修改文件
//
// 返回值:
//   - bool: 操作是否成功
//   - string: 成功时返回输出信息，失败时返回错误信息
func ResticBackup(restic_exe_path, resticPath, backupPath, passwd string, packSize int, compression, tag string, skip bool) (bool, string) {
	args := []string{"-r", resticPath, "backup", backupPath, "--no-scan"}

	if packSize >= 1 && packSize <= 128 {
		args = append(args, "--pack-size", strconv.Itoa(packSize))
	}

	if compression == "auto" || compression == "off" || compression == "max" {
		args = append(args, "--compression", compression)
	}

	if tag != "" {
		args = append(args, "--tag", tag)
	}

	if skip {
		args = append(args, "--skip-if-unchanged")
	}

	cmd := exec.Command(restic_exe_path, args...)
	stdin, err := cmd.StdinPipe()
	if err != nil {
		return false, fmt.Sprintf("Error creating stdin pipe: %v", err)
	}

	go func() {
		defer stdin.Close()
		io.WriteString(stdin, passwd+"\n")
	}()

	output, err := cmd.CombinedOutput()
	if err != nil {
		return false, fmt.Sprintf("Error: %v, Output: %s", err, output)
	}

	if cmd.ProcessState.ExitCode() == 0 {
		return true, string(output)
	}
	return false, string(output)
}

// ResBackup 检查并执行备份
// 参数:
//   - restic_exe_path: restic 可执行文件路径
//   - resticPath: restic 仓库路径
//   - backupPath: 需要备份的路径
//   - passwd: 仓库加密密码
//   - packSize: 分块大小(MB)，有效范围 1-128
//   - compression: 压缩模式（auto/off/max）
//   - tag: 备份标签
//   - skip: 是否跳过未修改文件
//
// 返回值:
//   - bool: 操作是否成功
//   - string: 成功时返回输出信息，失败时返回错误信息
//
// 功能说明:
//  1. 检查仓库是否存在，不存在则自动初始化
//  2. 执行实际备份操作
func ResBackup(restic_exe_path, resticPath, backupPath, passwd string, packSize int, compression, tag string, skip bool) (bool, string) {
	if _, err := os.Stat(resticPath); os.IsNotExist(err) {
		fmt.Printf("Restic repository %s not found, initializing...\n", resticPath)
		success, output := ResticInit(restic_exe_path, resticPath, passwd)
		if !success {
			return false, fmt.Sprintf("Init failed: %s", output)
		}
	} else {
		fmt.Printf("Restic repository %s already exists\n", resticPath)
	}

	fmt.Printf("Starting backup from %s to %s...\n", backupPath, resticPath)
	return ResticBackup(restic_exe_path, resticPath, backupPath, passwd, packSize, compression, tag, skip)
}

// ResClearFolder 清理restic仓库的data目录空文件夹
// 参数:
//   - path: restic仓库根路径
//
// 返回值:
//   - bool: 操作是否成功
//   - string: 成功时返回删除结果（格式："清除成功，共删除x个空文件夹，文件夹名分别是：name1，name2..."），
//     包含错误信息时会追加在成功信息之后
//
// 功能说明:
//   - 自动跳过非空目录
//   - 同时返回成功删除列表和错误信息
func ResClearFolder(path string) (bool, string) {
	dataDir := filepath.Join(path, "data")
	var deletedDirs []string
	var errorMsgs []string

	// 检查data目录是否存在
	if _, err := os.Stat(dataDir); os.IsNotExist(err) {
		return false, fmt.Sprintf("路径 %s 不是有效的restic仓库（缺少data目录）", path)
	}

	// 读取data目录下的所有子目录
	dirs, err := os.ReadDir(dataDir)
	if err != nil {
		return false, fmt.Sprintf("读取目录失败: %v", err)
	}

	// 遍历所有子目录
	for _, dir := range dirs {
		if dir.IsDir() {
			dirPath := filepath.Join(dataDir, dir.Name())

			// 检查是否为空目录
			isEmpty, err := isDirEmpty(dirPath)
			if err != nil {
				errorMsgs = append(errorMsgs, fmt.Sprintf("检查目录 %s 失败: %v", dir.Name(), err))
				continue
			}
			if isEmpty {
				// 删除空目录
				if err := os.Remove(dirPath); err == nil {
					deletedDirs = append(deletedDirs, dir.Name())
				} else {
					errorMsgs = append(errorMsgs, fmt.Sprintf("删除失败 %s: %v", dir.Name(), err))
				}
			}
		}
	}

	// 构建结果信息
	var result strings.Builder
	if len(deletedDirs) > 0 {
		result.WriteString(fmt.Sprintf("清除成功，共删除%d个空文件夹，文件夹名分别是：%s",
			len(deletedDirs), strings.Join(deletedDirs, "， ")))
	}
	if len(errorMsgs) > 0 {
		if result.Len() > 0 {
			result.WriteString("\n")
		}
		result.WriteString(strings.Join(errorMsgs, "\n"))
	}

	if result.Len() == 0 {
		return true, "未发现需要清理的空目录"
	}
	return true, result.String()
}

// 新增辅助函数检查目录是否为空
func isDirEmpty(path string) (bool, error) {
	f, err := os.Open(path)
	if err != nil {
		return false, err
	}
	defer f.Close()

	// 读取最多1个条目
	_, err = f.Readdir(1)
	if err == io.EOF {
		return true, nil // 目录为空
	}
	return false, err // 目录非空或读取错误
}

// ResticRestore 执行恢复操作
// 参数:
//   - resticPath: restic 仓库路径
//   - outputPath: 恢复文件输出路径
//   - passwd: 仓库加密密码
//   - snapshotID: 快照ID（支持"latest"或具体ID）
//
// 返回值:
//   - bool: 操作是否成功
//   - string: 成功时返回输出信息，失败时返回错误信息
//
// 功能流程:
//  1. 获取仓库快照列表
//  2. 解析并匹配指定快照
//  3. 执行恢复操作
func ResticRestore(resticPath, outputPath, passwd, snapshotID string) (bool, string) {
	// 获取快照列表
	cmdSnap := exec.Command("restic", "-r", resticPath, "snapshots", "--json")
	stdinSnap, err := cmdSnap.StdinPipe()
	if err != nil {
		return false, fmt.Sprintf("Error creating stdin pipe: %v", err)
	}

	go func() {
		defer stdinSnap.Close()
		io.WriteString(stdinSnap, passwd+"\n")
	}()

	outputSnap, err := cmdSnap.CombinedOutput()
	if err != nil {
		return false, fmt.Sprintf("Snapshots error: %v, Output: %s", err, outputSnap)
	}

	// 解析 JSON
	var snapshots []struct {
		ShortID string   `json:"short_id"`
		Paths   []string `json:"paths"`
	}

	// 提取 JSON 部分
	jsonStr := string(outputSnap)
	start := strings.Index(jsonStr, "[")
	if start == -1 {
		return false, "Invalid snapshots output"
	}
	jsonStr = jsonStr[start:]

	if err := json.Unmarshal([]byte(jsonStr), &snapshots); err != nil {
		return false, fmt.Sprintf("JSON parse error: %v", err)
	}

	// 查找匹配的快照
	var targetPath string
	found := false
	for _, snap := range snapshots {
		if snapshotID == "latest" || snap.ShortID == snapshotID {
			if len(snap.Paths) == 0 {
				continue
			}
			// 路径处理
			targetPath = strings.ReplaceAll(snap.Paths[0], "\\", "/")
			re := regexp.MustCompile(`^([a-zA-Z]):`)
			targetPath = re.ReplaceAllString(targetPath, "$1")
			targetPath = filepath.Dir(targetPath)
			found = true
			break
		}
	}

	if !found {
		return false, "Snapshot not found"
	}

	// 执行恢复
	args := []string{
		"-r", resticPath,
		"restore",
		fmt.Sprintf("%s:%s", snapshotID, targetPath),
		"--target", outputPath,
	}

	cmdRestore := exec.Command("restic", args...)
	stdinRestore, err := cmdRestore.StdinPipe()
	if err != nil {
		return false, fmt.Sprintf("Error creating stdin pipe: %v", err)
	}

	go func() {
		defer stdinRestore.Close()
		io.WriteString(stdinRestore, passwd+"\n")
	}()

	outputRestore, err := cmdRestore.CombinedOutput()
	if err != nil {
		return false, fmt.Sprintf("Restore error: %v, Output: %s", err, outputRestore)
	}

	if cmdRestore.ProcessState.ExitCode() == 0 {
		return true, string(outputRestore)
	}
	return false, string(outputRestore)
}

// CheckResticPath 检查系统中 restic 的可用性
// 返回值:
// - 1: 系统PATH中找到restic
// - 2: 当前程序目录找到restic.exe
// - 0: 未找到可用restic
func CheckResticPath() int {
	// 1. 检查系统PATH中的restic
	if path, err := exec.LookPath("restic"); err == nil {
		// 验证确实是restic程序
		cmd := exec.Command(path, "version")
		output, err := cmd.CombinedOutput()
		if err == nil && strings.Contains(string(output), "restic") {
			return 1
		}
	}

	// 2. 检查当前程序目录下的restic.exe
	exePath, err := os.Executable()
	if err == nil {
		exeDir := filepath.Dir(exePath)
		resticExePath := filepath.Join(exeDir, "restic.exe")
		if _, err := os.Stat(resticExePath); err == nil {
			return 2
		}
	}

	// 3. 未找到任何可用版本
	return 0
}
