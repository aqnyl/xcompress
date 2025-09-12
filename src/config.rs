use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct TomlConfig {
    pub name: Option<String>,
    pub path: Option<Vec<String>>,
    pub tag: Option<String>,
    pub passwd: Option<String>,
    pub restic_home_path: Option<String>,
    pub merge: Option<i64>,
    pub merge_name: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct GlobalConfig {
    pub merge: Option<i64>,
    pub passwd: Option<String>,
    pub restic_home_path: Option<String>,
    pub tag: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TomlConfigFile {
    #[serde(default)]
    global_config: GlobalConfig,
    config: HashMap<String, TomlConfig>,
}

#[derive(Debug, Clone)]
pub struct FinalConfig {
    pub key_name: String,
    pub name: String,
    pub path: Vec<String>,
    pub tag: String,
    pub passwd: String,
    pub restic_home_path: String,
    pub merge: i64,
    pub merge_name: String,
}

/// 解析 TOML 配置文件并验证
pub fn parse_toml(file_path: &str) -> Result<Vec<FinalConfig>, String> {
    let toml_content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("读取 TOML 文件 '{}' 失败: {}", file_path, e))?;
    
    let config_file: TomlConfigFile = toml::from_str(&toml_content)
        .map_err(|e| format!("TOML 文件解析失败: {}", e))?;

    let mut final_configs = Vec::new();
    let mut error_messages = String::new();

    for (key_name, cfg) in config_file.config {
        let mut final_cfg = FinalConfig {
            key_name: key_name.clone(),
            name: cfg.name.unwrap_or_else(|| key_name.clone()),
            path: cfg.path.unwrap_or_default(),
            tag: cfg.tag.or(config_file.global_config.tag.clone()).unwrap_or_default(),
            passwd: cfg.passwd.or(config_file.global_config.passwd.clone()).unwrap_or_default(),
            restic_home_path: cfg.restic_home_path.or(config_file.global_config.restic_home_path.clone()).unwrap_or_default(),
            merge: cfg.merge.unwrap_or_else(|| config_file.global_config.merge.unwrap_or(0)),
            merge_name: cfg.merge_name.unwrap_or_else(|| "merged_backup".to_string()),
        };

        // 智能判断 merge 默认值
        if final_cfg.path.len() > 1 && cfg.merge.is_none() && config_file.global_config.merge.is_none() {
            final_cfg.merge = 1;
        }

        // 验证必填字段
        if final_cfg.path.is_empty() {
            error_messages.push_str(&format!("[{}]: `path` 字段不能为空。\n", key_name));
        }
        if final_cfg.passwd.is_empty() {
            error_messages.push_str(&format!("[{}]: `passwd` 字段不能为空 (全局或局部必须设置一个)。\n", key_name));
        }
        if final_cfg.restic_home_path.is_empty() {
            error_messages.push_str(&format!("[{}]: `restic_home_path` 字段不能为空 (全局或局部必须设置一个)。\n", key_name));
        }
        if final_cfg.merge != 0 && final_cfg.merge != 1 {
            error_messages.push_str(&format!("[{}]: `merge` 字段必须为 0 或 1。\n", key_name));
        }

        // 验证路径存在性
        for p in &final_cfg.path {
            if !Path::new(p).exists() {
                error_messages.push_str(&format!("[{}]: 备份源路径 '{}' 不存在。\n", key_name, p));
            }
        }
        
        final_configs.push(final_cfg);
    }

    if !error_messages.is_empty() {
        return Err(format!("配置文件验证失败:\n{}", error_messages));
    }

    if final_configs.is_empty() {
        return Err("配置文件中未找到任何有效的 [config] 配置项。".to_string());
    }
    
    Ok(final_configs)
}