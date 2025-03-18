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
}

// 修改顶层配置结构
type TomlConfigFile struct {
	Configs map[string]TomlConfig `toml:"config"` // 添加新的顶层配置结构
}

// Toml_parse TOML配置文件解析与验证函数
// 参数:
//
//	filePath - TOML配置文件的完整路径
//
// 返回值:
//
//	bool - 验证是否通过(true表示验证成功)
//	interface{} - 验证成功时返回配置字典，失败时返回错误信息字符串
func Toml_parse(filePath string) (bool, interface{}) {
	var configFile TomlConfigFile
	var errorMessages string

	// 解析TOML文件
	if _, err := toml.DecodeFile(filePath, &configFile); err != nil {
		return false, fmt.Sprintf("TOML解析错误: %v", err)
	}

	// 遍历所有配置项（改为使用map的遍历方式）
	for configName, cfg := range configFile.Configs {
		// 验证必填字段（将i+1改为配置项名称）
		if cfg.Name == "" {
			errorMessages += fmt.Sprintf("配置项[%s]: name字段缺失\n", configName)
		}
		if len(cfg.Path) == 0 {
			errorMessages += fmt.Sprintf("配置项[%s]: path字段缺失\n", configName)
		}
		if cfg.Passwd == "" {
			errorMessages += fmt.Sprintf("配置项[%s]: passwd字段缺失\n", configName)
		}
		if cfg.ResticHomePath == "" {
			errorMessages += fmt.Sprintf("配置项[%s]: restic_home_path字段缺失\n", configName)
		}

		// 验证路径存在性
		for _, p := range cfg.Path {
			if !pathExists(p) {
				errorMessages += fmt.Sprintf("配置项[%s]: 路径不存在 - %s\n", configName, p)
			}
		}
		if !pathExists(cfg.ResticHomePath) {
			errorMessages += fmt.Sprintf("配置项[%s]: restic仓库路径不存在 - %s\n", configName, cfg.ResticHomePath)
		}

		// 新增 merge 字段验证
		if cfg.Merge != 0 && cfg.Merge != 1 {
			errorMessages += fmt.Sprintf("配置项[%s]: merge字段必须为0或1\n", configName)
		}
	}

	if errorMessages != "" {
		return false, errorMessages
	}
	// 验证通过时返回配置字典
	return true, configFile.Configs
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
//   - resticPath: restic 仓库路径
//   - passwd: 仓库加密密码
//
// 返回值:
//   - bool: 操作是否成功
//   - string: 成功时返回输出信息，失败时返回错误信息
func ResticInit(resticPath, passwd string) (bool, string) {
	cmd := exec.Command("restic", "init", "-r", resticPath, "--repository-version", "2")
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
func ResticBackup(resticPath, backupPath, passwd string, packSize int, compression, tag string, skip bool) (bool, string) {
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

	cmd := exec.Command("restic", args...)
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
func ResBackup(resticPath, backupPath, passwd string, packSize int, compression, tag string, skip bool) (bool, string) {
	if _, err := os.Stat(resticPath); os.IsNotExist(err) {
		fmt.Printf("Restic repository %s not found, initializing...\n", resticPath)
		success, output := ResticInit(resticPath, passwd)
		if !success {
			return false, fmt.Sprintf("Init failed: %s", output)
		}
	} else {
		fmt.Printf("Restic repository %s already exists\n", resticPath)
	}

	fmt.Printf("Starting backup from %s to %s...\n", backupPath, resticPath)
	return ResticBackup(resticPath, backupPath, passwd, packSize, compression, tag, skip)
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
