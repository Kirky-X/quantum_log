//! 环境变量配置模块
//!
//! 此模块提供从环境变量和配置文件安全读取敏感凭证的功能。

use std::env;
use std::fs;
use std::path::Path;
use crate::error::{QuantumLogError, Result};

/// 环境变量配置管理器
pub struct EnvConfig;

impl EnvConfig {
    /// 从环境变量或文件读取InfluxDB token
    pub fn get_influxdb_token() -> Result<Option<String>> {
        // 首先尝试从环境变量读取
        if let Ok(token) = env::var("INFLUXDB_TOKEN") {
            if !token.is_empty() {
                return Ok(Some(token));
            }
        }
        
        // 然后尝试从account.md文件读取
        let account_file = Path::new("account.md");
        if account_file.exists() {
            match fs::read_to_string(account_file) {
                Ok(content) => {
                    let token = content.trim().to_string();
                    if !token.is_empty() {
                        return Ok(Some(token));
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read account.md: {}", e);
                }
            }
        }
        
        Ok(None)
    }
    
    /// 从环境变量读取InfluxDB用户名
    pub fn get_influxdb_username() -> Option<String> {
        env::var("INFLUXDB_USERNAME").ok().filter(|s| !s.is_empty())
    }
    
    /// 从环境变量读取InfluxDB密码
    pub fn get_influxdb_password() -> Option<String> {
        env::var("INFLUXDB_PASSWORD").ok().filter(|s| !s.is_empty())
    }
    
    /// 从环境变量读取InfluxDB URL
    pub fn get_influxdb_url() -> Option<String> {
        env::var("INFLUXDB_URL").ok().filter(|s| !s.is_empty())
    }
    
    /// 从环境变量读取InfluxDB数据库名
    pub fn get_influxdb_database() -> Option<String> {
        env::var("INFLUXDB_DATABASE").ok().filter(|s| !s.is_empty())
    }
    
    /// 从环境变量读取InfluxDB组织名
    pub fn get_influxdb_org() -> Option<String> {
        env::var("INFLUXDB_ORG").ok().filter(|s| !s.is_empty())
    }
    
    /// 从环境变量读取InfluxDB bucket名
    pub fn get_influxdb_bucket() -> Option<String> {
        env::var("INFLUXDB_BUCKET").ok().filter(|s| !s.is_empty())
    }
    
    /// 验证InfluxDB配置的完整性
    pub fn validate_influxdb_config() -> Result<()> {
        let has_token = Self::get_influxdb_token()?.is_some();
        let has_username = Self::get_influxdb_username().is_some();
        let has_password = Self::get_influxdb_password().is_some();
        
        // 必须有token或者用户名密码组合
        if !has_token && !(has_username && has_password) {
            return Err(QuantumLogError::ConfigError(
                "InfluxDB authentication required: either INFLUXDB_TOKEN or both INFLUXDB_USERNAME and INFLUXDB_PASSWORD must be set".to_string()
            ));
        }
        
        Ok(())
    }
}

/// 安全地从配置中获取凭证，优先使用环境变量
pub fn get_secure_influxdb_config() -> Result<(Option<String>, Option<String>, Option<String>)> {
    let token = EnvConfig::get_influxdb_token()?;
    let username = EnvConfig::get_influxdb_username();
    let password = EnvConfig::get_influxdb_password();
    
    Ok((token, username, password))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    
    #[test]
    fn test_env_config_token() {
        // 设置测试环境变量
        env::set_var("INFLUXDB_TOKEN", "test_token_123");
        
        let token = EnvConfig::get_influxdb_token().unwrap();
        assert_eq!(token, Some("test_token_123".to_string()));
        
        // 清理
        env::remove_var("INFLUXDB_TOKEN");
    }
    
    #[test]
    fn test_env_config_username_password() {
        // 设置测试环境变量
        env::set_var("INFLUXDB_USERNAME", "test_user");
        env::set_var("INFLUXDB_PASSWORD", "test_pass");
        
        let username = EnvConfig::get_influxdb_username();
        let password = EnvConfig::get_influxdb_password();
        
        assert_eq!(username, Some("test_user".to_string()));
        assert_eq!(password, Some("test_pass".to_string()));
        
        // 清理
        env::remove_var("INFLUXDB_USERNAME");
        env::remove_var("INFLUXDB_PASSWORD");
    }
}