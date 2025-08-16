//! QuantumLog 示例测试脚本
//! 这个脚本用于验证所有示例代码的正确性和可运行性

use std::process::Command;
use std::path::Path;
use std::fs;

/// 测试结果
#[derive(Debug)]
struct TestResult {
    name: String,
    success: bool,
    output: String,
    error: Option<String>,
}

/// 示例信息
#[derive(Debug)]
struct ExampleInfo {
    name: String,
    file: String,
    description: String,
}

/// 获取所有示例
fn get_examples() -> Vec<ExampleInfo> {
    vec![
        ExampleInfo {
            name: "基本使用示例".to_string(),
            file: "basic_usage".to_string(),
            description: "演示 QuantumLog 的基本使用方法".to_string(),
        },
        ExampleInfo {
            name: "完整示例集合".to_string(),
            file: "complete_examples".to_string(),
            description: "包含所有使用场景的完整示例".to_string(),
        },
        ExampleInfo {
            name: "配置文件示例".to_string(),
            file: "config_file_example".to_string(),
            description: "演示如何从配置文件加载设置".to_string(),
        },
        // 新增：统一 Sink Trait 使用示例（包含对独占型 sink 和构建器 API 的测试）
        ExampleInfo {
            name: "统一 Sink Trait 使用示例（含单元测试）".to_string(),
            file: "sink_trait_usage".to_string(),
            description: "覆盖 Pipeline::add_exclusive_sink 与 PipelineBuilder::set_exclusive_sink 的示例与测试".to_string(),
        },
    ]
}

/// 运行单个示例
fn run_example(example: &ExampleInfo) -> TestResult {
    println!("\n🧪 测试示例: {}", example.name);
    println!("📁 文件: examples/{}.rs", example.file);
    println!("📝 描述: {}", example.description);
    println!("{}", "=".repeat(60));

    // 特殊处理：sink_trait_usage 我们运行其示例内的单元测试，以执行对独占型 sink/构建器 API 的覆盖
    let output = if example.file == "sink_trait_usage" {
        Command::new("cargo")
            .args(["test", "--example", &example.file])
            .output()
    } else {
        Command::new("cargo")
            .args(["run", "--example", &example.file])
            .output()
    };

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);

            let success = output.status.success();
            let combined_output = format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr);

            TestResult {
                name: example.name.clone(),
                success,
                output: combined_output,
                error: if success { None } else { Some(stderr.to_string()) },
            }
        }
        Err(e) => TestResult {
            name: example.name.clone(),
            success: false,
            output: String::new(),
            error: Some(format!("执行命令失败: {}", e)),
        },
    }
}

/// 检查项目环境
fn check_environment() -> Result<(), String> {
    // 检查是否在正确的目录
    if !Path::new("Cargo.toml").exists() {
        return Err("请在 QuantumLog 项目根目录运行此脚本".to_string());
    }

    // 检查 cargo 是否可用
    let cargo_check = Command::new("cargo").args(["--version"]).output();

    match cargo_check {
        Ok(output) => {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                println!("✅ Cargo 版本: {}", version.trim());
            } else {
                return Err("Cargo 不可用".to_string());
            }
        }
        Err(e) => {
            return Err(format!("无法执行 cargo: {}", e));
        }
    }

    // 检查项目是否可以编译
    println!("🔍 检查项目编译状态...");
    let build_check = Command::new("cargo").args(["check"]).output();

    match build_check {
        Ok(output) => {
            if output.status.success() {
                println!("✅ 项目编译检查通过");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("项目编译检查失败:\n{}", stderr));
            }
        }
        Err(e) => {
            return Err(format!("无法执行编译检查: {}", e));
        }
    }

    Ok(())
}

/// 创建必要的目录
fn setup_directories() -> Result<(), String> {
    // 创建日志目录
    if !Path::new("logs").exists() {
        fs::create_dir_all("logs").map_err(|e| format!("创建日志目录失败: {}", e))?;
        println!("📁 创建日志目录: logs/");
    }

    Ok(())
}

/// 生成测试报告
fn generate_report(results: &[TestResult]) -> String {
    let mut report = String::new();
    for r in results {
        let error_snippet = r
            .error
            .as_ref()
            .map(|e| e.chars().take(200).collect::<String>())
            .unwrap_or_else(|| "<none>".to_string());

        report.push_str(&format!(
            "Example: {} -> {}\nOutput snippet: {}\nError: {}\n\n",
            r.name,
            if r.success { "SUCCESS" } else { "FAIL" },
            // 限制输出片段长度，避免报告过长
            r.output.chars().take(200).collect::<String>(),
            error_snippet,
        ));
    }
    report
}

/// 清理函数
fn cleanup() {
    println!("\n🧹 清理临时文件...");

    // 清理可能的临时配置文件
    let temp_files = ["temp_config.toml"]; 

    for file in &temp_files {
        if Path::new(file).exists() {
            if let Err(e) = fs::remove_file(file) {
                println!("⚠️  删除临时文件 {} 失败: {}", file, e);
            } else {
                println!("🗑️  删除临时文件: {}", file);
            }
        }
    }
}

/// 主函数
fn main() {
    println!("🚀 QuantumLog 示例测试脚本");
    println!("{}", "=".repeat(50));

    // 检查环境
    if let Err(e) = check_environment() {
        eprintln!("❌ 环境检查失败: {}", e);
        std::process::exit(1);
    }

    // 设置目录
    if let Err(e) = setup_directories() {
        eprintln!("❌ 目录设置失败: {}", e);
        std::process::exit(1);
    }

    // 获取所有示例
    let examples = get_examples();
    println!("\n📝 找到 {} 个示例需要测试", examples.len());

    // 运行所有示例
    let mut results = Vec::new();

    for example in &examples {
        let result = run_example(example);

        if result.success {
            println!("✅ {} 测试通过", result.name);
        } else {
            println!("❌ {} 测试失败", result.name);
        }

        results.push(result);

        // 在测试之间稍作停顿
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    // 生成并打印报告，确保使用到 output 字段以避免 dead_code 警告
    let report = generate_report(&results);
    println!("{}", report);

    // 清理
    cleanup();

    // 根据结果设置退出码
    let failed_count = results.iter().filter(|r| !r.success).count();
    if failed_count > 0 {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_get_examples() {
        let examples = get_examples();
        assert!(!examples.is_empty());
        assert!(examples.iter().any(|e| e.file == "basic_usage"));
        assert!(examples.iter().any(|e| e.file == "complete_examples"));
        assert!(examples.iter().any(|e| e.file == "config_file_example"));
    }
    
    #[test]
    fn test_example_info_creation() {
        let example = ExampleInfo {
            name: "测试示例".to_string(),
            file: "test_example".to_string(),
            description: "这是一个测试示例".to_string(),
        };
        
        assert_eq!(example.name, "测试示例");
        assert_eq!(example.file, "test_example");
        assert_eq!(example.description, "这是一个测试示例");
    }
    
    #[test]
    fn test_test_result_creation() {
        let result = TestResult {
            name: "测试结果".to_string(),
            success: true,
            output: "测试输出".to_string(),
            error: None,
        };
        
        assert_eq!(result.name, "测试结果");
        assert!(result.success);
        assert_eq!(result.output, "测试输出");
        assert!(result.error.is_none());
    }
}