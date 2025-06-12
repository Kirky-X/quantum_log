# QuantumLog 示例运行脚本
# 这个 PowerShell 脚本帮助用户快速运行所有示例

Write-Host "QuantumLog 示例运行脚本" -ForegroundColor Green
Write-Host "========================" -ForegroundColor Green
Write-Host ""

# 检查是否在正确的目录
if (-not (Test-Path "Cargo.toml")) {
    Write-Host "错误: 请在 QuantumLog 项目根目录运行此脚本" -ForegroundColor Red
    exit 1
}

# 创建日志目录
if (-not (Test-Path "logs")) {
    New-Item -ItemType Directory -Path "logs" | Out-Null
    Write-Host "创建日志目录: logs/" -ForegroundColor Yellow
}

# 示例列表
$examples = @(
    @{Name="基本使用示例"; File="basic_usage"; Description="演示 QuantumLog 的基本使用方法"},
    @{Name="完整示例集合"; File="complete_examples"; Description="包含所有使用场景的完整示例"},
    @{Name="配置文件示例"; File="config_file_example"; Description="演示如何从配置文件加载设置"}
)

# 运行示例的函数
function Run-Example {
    param(
        [string]$Name,
        [string]$File,
        [string]$Description
    )
    
    Write-Host "\n运行示例: $Name" -ForegroundColor Cyan
    Write-Host "描述: $Description" -ForegroundColor Gray
    Write-Host "文件: examples/$File.rs" -ForegroundColor Gray
    Write-Host "$('=' * 60)" -ForegroundColor Cyan
    
    try {
        # 运行示例
        $result = cargo run --example $File 2>&1
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "✅ $Name 运行成功" -ForegroundColor Green
            # 显示输出的最后几行
            $result | Select-Object -Last 5 | ForEach-Object {
                Write-Host "   $_" -ForegroundColor White
            }
        } else {
            Write-Host "❌ $Name 运行失败" -ForegroundColor Red
            Write-Host "错误信息:" -ForegroundColor Red
            $result | ForEach-Object {
                Write-Host "   $_" -ForegroundColor Red
            }
        }
    }
    catch {
        Write-Host "❌ $Name 运行时发生异常: $($_.Exception.Message)" -ForegroundColor Red
    }
}

# 主菜单
function Show-Menu {
    Write-Host "\n请选择要运行的示例:" -ForegroundColor Yellow
    Write-Host "0. 运行所有示例" -ForegroundColor White
    
    for ($i = 0; $i -lt $examples.Count; $i++) {
        $example = $examples[$i]
        Write-Host "$($i + 1). $($example.Name)" -ForegroundColor White
        Write-Host "   $($example.Description)" -ForegroundColor Gray
    }
    
    Write-Host "q. 退出" -ForegroundColor White
    Write-Host ""
}

# 运行所有示例
function Run-AllExamples {
    Write-Host "\n🚀 运行所有示例..." -ForegroundColor Green
    
    foreach ($example in $examples) {
        Run-Example -Name $example.Name -File $example.File -Description $example.Description
        Start-Sleep -Seconds 1
    }
    
    Write-Host "\n🎉 所有示例运行完成!" -ForegroundColor Green
}

# 检查依赖
function Check-Dependencies {
    Write-Host "检查依赖..." -ForegroundColor Yellow
    
    # 检查 Rust 和 Cargo
    try {
        $rustVersion = cargo --version 2>$null
        Write-Host "✅ Cargo: $rustVersion" -ForegroundColor Green
    }
    catch {
        Write-Host "❌ 未找到 Cargo，请安装 Rust" -ForegroundColor Red
        exit 1
    }
    
    # 检查项目是否可以编译
    Write-Host "检查项目编译..." -ForegroundColor Yellow
    $buildResult = cargo check 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ 项目编译检查通过" -ForegroundColor Green
    } else {
        Write-Host "❌ 项目编译检查失败" -ForegroundColor Red
        Write-Host "错误信息:" -ForegroundColor Red
        $buildResult | ForEach-Object {
            Write-Host "   $_" -ForegroundColor Red
        }
        Write-Host "\n请先解决编译错误再运行示例" -ForegroundColor Yellow
        exit 1
    }
}

# 清理函数
function Clean-Up {
    Write-Host "\n清理临时文件..." -ForegroundColor Yellow
    
    # 清理可能的临时配置文件
    if (Test-Path "temp_config.toml") {
        Remove-Item "temp_config.toml" -Force
        Write-Host "删除临时配置文件" -ForegroundColor Gray
    }
    
    # 可选：清理日志文件
    $cleanLogs = Read-Host "是否清理日志文件? (y/N)"
    if ($cleanLogs -eq 'y' -or $cleanLogs -eq 'Y') {
        if (Test-Path "logs") {
            Remove-Item "logs" -Recurse -Force
            Write-Host "日志文件已清理" -ForegroundColor Gray
        }
    }
}

# 主程序
try {
    # 检查依赖
    Check-Dependencies
    
    # 如果有命令行参数，直接运行
    if ($args.Count -gt 0) {
        switch ($args[0]) {
            "all" {
                Run-AllExamples
                exit 0
            }
            "basic" {
                Run-Example -Name $examples[0].Name -File $examples[0].File -Description $examples[0].Description
                exit 0
            }
            "complete" {
                Run-Example -Name $examples[1].Name -File $examples[1].File -Description $examples[1].Description
                exit 0
            }
            "config" {
                Run-Example -Name $examples[2].Name -File $examples[2].File -Description $examples[2].Description
                exit 0
            }
            "clean" {
                Clean-Up
                exit 0
            }
            default {
                Write-Host "未知参数: $($args[0])" -ForegroundColor Red
                Write-Host "可用参数: all, basic, complete, config, clean" -ForegroundColor Yellow
                exit 1
            }
        }
    }
    
    # 交互式菜单
    while ($true) {
        Show-Menu
        $choice = Read-Host "请输入选择"
        
        switch ($choice) {
            "0" {
                Run-AllExamples
            }
            "1" {
                Run-Example -Name $examples[0].Name -File $examples[0].File -Description $examples[0].Description
            }
            "2" {
                Run-Example -Name $examples[1].Name -File $examples[1].File -Description $examples[1].Description
            }
            "3" {
                Run-Example -Name $examples[2].Name -File $examples[2].File -Description $examples[2].Description
            }
            "q" {
                Write-Host "\n再见! 👋" -ForegroundColor Green
                break
            }
            default {
                Write-Host "无效选择，请重新输入" -ForegroundColor Red
            }
        }
    }
}
finally {
    # 可选清理
    $cleanup = Read-Host "\n是否进行清理? (y/N)"
    if ($cleanup -eq 'y' -or $cleanup -eq 'Y') {
        Clean-Up
    }
}

Write-Host "\n脚本执行完成" -ForegroundColor Green