# QuantumLog Examples Runner Script
# PowerShell script to run all examples and tests

Write-Host "QuantumLog Examples Runner" -ForegroundColor Green
Write-Host "========================" -ForegroundColor Green
Write-Host ""

# Check if we're in the correct directory
if (-not (Test-Path "Cargo.toml")) {
    Write-Host "Error: Please run this script from the QuantumLog project root directory" -ForegroundColor Red
    exit 1
}

# Create logs directory if it doesn't exist
if (-not (Test-Path "logs")) {
    New-Item -ItemType Directory -Path "logs" | Out-Null
    Write-Host "Created logs directory: logs/" -ForegroundColor Yellow
}

# Example configurations
$examples = @(
    @{Name="Basic Usage Example"; File="basic_usage"; Description="Demonstrates basic QuantumLog usage"},
    @{Name="Complete Examples"; File="complete_examples"; Description="Comprehensive usage examples"},
    @{Name="Config File Example"; File="config_file_example"; Description="Configuration file loading example"}
)

# Database feature tests (mutually exclusive)
$databaseFeatures = @(
    @{Name="SQLite Database Test"; Feature="sqlite"},
    @{Name="PostgreSQL Database Test"; Feature="postgres"},
    @{Name="MySQL Database Test"; Feature="mysql"}
)

# Function to run an example
function Run-Example {
    param(
        [string]$Name,
        [string]$File,
        [string]$Description,
        [string]$Feature = ""
    )
    
    Write-Host "`nRunning example: $Name" -ForegroundColor Cyan
    Write-Host "Description: $Description" -ForegroundColor Gray
    Write-Host "File: examples/$File.rs" -ForegroundColor Gray
    
    if ($Feature) {
        Write-Host "Feature: $Feature" -ForegroundColor Yellow
    }
    
    Write-Host "$('=' * 60)" -ForegroundColor Cyan
    
    # Build cargo command
    $cargoArgs = @("run", "--example", $File)
    if ($Feature) {
        $cargoArgs += "--features"
        $cargoArgs += $Feature
    }
    
    # Run the example
    try {
        $result = & cargo @cargoArgs 2>&1
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "✅ $Name succeeded" -ForegroundColor Green
            if ($result) {
                $result | Select-Object -Last 3 | ForEach-Object {
                    Write-Host "   $_" -ForegroundColor White
                }
            }
            return $true
        } else {
            Write-Host "❌ $Name failed" -ForegroundColor Red
            if ($result) {
                $result | Select-Object -Last 5 | ForEach-Object {
                    Write-Host "   $_" -ForegroundColor Red
                }
            }
            return $false
        }
    }
    catch {
        Write-Host "❌ $Name exception: $($_.Exception.Message)" -ForegroundColor Red
        return $false
    }
}

# Function to run tests with features
function Run-Test {
    param(
        [string]$Name,
        [string]$Feature = ""
    )
    
    Write-Host "`nRunning test: $Name" -ForegroundColor Cyan
    
    if ($Feature) {
        Write-Host "Feature: $Feature" -ForegroundColor Yellow
    }
    
    Write-Host "$('=' * 60)" -ForegroundColor Cyan
    
    # Build cargo command
    $cargoArgs = @("test")
    if ($Feature) {
        $cargoArgs += "--features"
        $cargoArgs += $Feature
    }
    
    try {
        $result = & cargo @cargoArgs 2>&1
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "✅ $Name tests passed" -ForegroundColor Green
            return $true
        } else {
            Write-Host "❌ $Name tests failed" -ForegroundColor Red
            if ($result) {
                $result | Select-Object -Last 8 | ForEach-Object {
                    Write-Host "   $_" -ForegroundColor Red
                }
            }
            return $false
        }
    }
    catch {
        Write-Host "❌ $Name test exception: $($_.Exception.Message)" -ForegroundColor Red
        return $false
    }
}

# Function to run all examples
function Run-AllExamples {
    Write-Host "`n🚀 Running all examples..." -ForegroundColor Green
    
    $successCount = 0
    $totalCount = $examples.Count
    
    foreach ($example in $examples) {
        $success = Run-Example -Name $example.Name -File $example.File -Description $example.Description
        if ($success) {
            $successCount++
        }
        Start-Sleep -Seconds 1
    }
    
    Write-Host "`n🎉 Examples completed! ($successCount/$totalCount succeeded)" -ForegroundColor Green
    
    if ($successCount -eq $totalCount) {
        Write-Host "All examples succeeded! 🎊" -ForegroundColor Green
    } else {
        Write-Host "$($totalCount - $successCount) examples failed" -ForegroundColor Yellow
    }
    
    return ($successCount -eq $totalCount)
}

# Function to run database tests
function Run-DatabaseTests {
    Write-Host "`n🗄️ Running database feature tests..." -ForegroundColor Green
    Write-Host "Note: Database features are mutually exclusive" -ForegroundColor Yellow
    
    $successCount = 0
    $totalCount = $databaseFeatures.Count
    
    foreach ($dbTest in $databaseFeatures) {
        $success = Run-Test -Name $dbTest.Name -Feature $dbTest.Feature
        if ($success) {
            $successCount++
        }
        Start-Sleep -Seconds 2
    }
    
    Write-Host "`n🎉 Database tests completed! ($successCount/$totalCount succeeded)" -ForegroundColor Green
    return ($successCount -eq $totalCount)
}

# Function to clean up temporary files
function Clean-Up {
    Write-Host "`n🧹 Cleaning temporary files..." -ForegroundColor Yellow
    
    # Clean possible temporary config files
    if (Test-Path "temp_config.toml") {
        Remove-Item "temp_config.toml" -Force
        Write-Host "Deleted temporary config file" -ForegroundColor Gray
    }
    
    # Optional: clean log files
    $response = Read-Host "Do you want to clean log files? (y/N)"
    if ($response -match "^[Yy]$") {
        if (Test-Path "logs") {
            Remove-Item "logs" -Recurse -Force
            Write-Host "Log files cleaned" -ForegroundColor Gray
        }
    }
}

# Function to show help information
function Show-Help {
    Write-Host "Usage: .\run_examples.ps1 [option]"
    Write-Host ""
    Write-Host "Options:"
    Write-Host "  all       Run all examples"
    Write-Host "  basic     Run basic usage example"
    Write-Host "  complete  Run complete example collection"
    Write-Host "  config    Run config file example"
    Write-Host "  test      Run unit tests"
    Write-Host "  database  Run database feature tests (SQLite, PostgreSQL, MySQL)"
    Write-Host "  clean     Clean temporary files and logs"
    Write-Host "  help      Show this help information"
    Write-Host ""
    Write-Host "Running without parameters will show interactive menu"
}

# Function to check dependencies
function Check-Dependencies {
    Write-Host "Checking dependencies..." -ForegroundColor Yellow
    
    # Check Rust and Cargo
    try {
        $rustVersion = cargo --version 2>$null
        Write-Host "✅ Cargo: $rustVersion" -ForegroundColor Green
    }
    catch {
        Write-Host "❌ Cargo not found, please install Rust" -ForegroundColor Red
        Write-Host "Installation: https://rustup.rs/" -ForegroundColor Yellow
        exit 1
    }
    
    # Check if project compiles
    Write-Host "Checking project compilation..." -ForegroundColor Yellow
    $buildResult = cargo check 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ Project compilation check passed" -ForegroundColor Green
    } else {
        Write-Host "❌ Project compilation check failed" -ForegroundColor Red
        Write-Host "Error details:" -ForegroundColor Red
        $buildResult | Select-Object -Last 10 | ForEach-Object {
            Write-Host "   $_" -ForegroundColor Red
        }
        Write-Host "`nPlease fix compilation errors before running examples" -ForegroundColor Yellow
        exit 1
    }
}

# Main execution
try {
    # Check dependencies first
    Check-Dependencies
    
    # Handle command line arguments
    if ($args.Count -gt 0) {
        switch ($args[0]) {
            "all" {
                $success = Run-AllExamples
                exit $(if ($success) { 0 } else { 1 })
            }
            "basic" {
                $success = Run-Example -Name $examples[0].Name -File $examples[0].File -Description $examples[0].Description
                exit $(if ($success) { 0 } else { 1 })
            }
            "complete" {
                $success = Run-Example -Name $examples[1].Name -File $examples[1].File -Description $examples[1].Description
                exit $(if ($success) { 0 } else { 1 })
            }
            "config" {
                $success = Run-Example -Name $examples[2].Name -File $examples[2].File -Description $examples[2].Description
                exit $(if ($success) { 0 } else { 1 })
            }
            "database" {
                $success = Run-DatabaseTests
                exit $(if ($success) { 0 } else { 1 })
            }
            "test" {
                $success = Run-Test -Name "All Unit Tests"
                exit $(if ($success) { 0 } else { 1 })
            }
            "clean" {
                Clean-Up
                exit 0
            }
            "help" {
                Show-Help
                exit 0
            }
            default {
                Write-Host "Unknown argument: $($args[0])" -ForegroundColor Red
                Write-Host "Available arguments: all, basic, complete, config, database, test, clean, help" -ForegroundColor Yellow
                exit 1
            }
        }
    }
    
    # Interactive menu
    while ($true) {
        Write-Host "`nPlease select an option:" -ForegroundColor Yellow
        Write-Host "0. Run all examples" -ForegroundColor White
        Write-Host "1. Basic Usage Example" -ForegroundColor White
        Write-Host "2. Complete Examples" -ForegroundColor White
        Write-Host "3. Config File Example" -ForegroundColor White
        Write-Host "d. Run database feature tests" -ForegroundColor White
        Write-Host "t. Run all unit tests" -ForegroundColor White
        Write-Host "q. Quit" -ForegroundColor White
        Write-Host ""
        
        $choice = Read-Host "Enter your choice"
        
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
            "d" {
                Run-DatabaseTests
            }
            "t" {
                Run-Test -Name "All Unit Tests"
            }
            "q" {
                Write-Host "`nGoodbye! 👋" -ForegroundColor Green
                break
            }
            default {
                Write-Host "Invalid choice, please try again" -ForegroundColor Red
            }
        }
    }
}
catch {
    Write-Host "Script execution error: $($_.Exception.Message)" -ForegroundColor Red
    exit 1
}

Write-Host "`nScript execution completed" -ForegroundColor Green