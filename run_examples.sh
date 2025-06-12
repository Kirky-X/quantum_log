#!/bin/bash

# QuantumLog 示例运行脚本
# 这个 Bash 脚本帮助用户快速运行所有示例

set -e  # 遇到错误时退出

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
GRAY='\033[0;37m'
WHITE='\033[1;37m'
NC='\033[0m' # No Color

echo -e "${GREEN}QuantumLog 示例运行脚本${NC}"
echo -e "${GREEN}========================${NC}"
echo ""

# 检查是否在正确的目录
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}错误: 请在 QuantumLog 项目根目录运行此脚本${NC}"
    exit 1
fi

# 创建日志目录
if [ ! -d "logs" ]; then
    mkdir -p logs
    echo -e "${YELLOW}创建日志目录: logs/${NC}"
fi

# 示例列表
declare -a examples=(
    "basic_usage:基本使用示例:演示 QuantumLog 的基本使用方法"
    "complete_examples:完整示例集合:包含所有使用场景的完整示例"
    "config_file_example:配置文件示例:演示如何从配置文件加载设置"
)

# 运行示例的函数
run_example() {
    local file="$1"
    local name="$2"
    local description="$3"
    
    echo -e "\n${CYAN}运行示例: $name${NC}"
    echo -e "${GRAY}描述: $description${NC}"
    echo -e "${GRAY}文件: examples/$file.rs${NC}"
    echo -e "${CYAN}$(printf '=%.0s' {1..60})${NC}"
    
    if cargo run --example "$file" 2>&1; then
        echo -e "${GREEN}✅ $name 运行成功${NC}"
    else
        echo -e "${RED}❌ $name 运行失败${NC}"
        return 1
    fi
}

# 主菜单
show_menu() {
    echo -e "\n${YELLOW}请选择要运行的示例:${NC}"
    echo -e "${WHITE}0. 运行所有示例${NC}"
    
    local i=1
    for example in "${examples[@]}"; do
        IFS=':' read -r file name description <<< "$example"
        echo -e "${WHITE}$i. $name${NC}"
        echo -e "   ${GRAY}$description${NC}"
        ((i++))
    done
    
    echo -e "${WHITE}q. 退出${NC}"
    echo ""
}

# 运行所有示例
run_all_examples() {
    echo -e "\n${GREEN}🚀 运行所有示例...${NC}"
    
    local success_count=0
    local total_count=${#examples[@]}
    
    for example in "${examples[@]}"; do
        IFS=':' read -r file name description <<< "$example"
        if run_example "$file" "$name" "$description"; then
            ((success_count++))
        fi
        sleep 1
    done
    
    echo -e "\n${GREEN}🎉 示例运行完成! ($success_count/$total_count 成功)${NC}"
    
    if [ $success_count -eq $total_count ]; then
        echo -e "${GREEN}所有示例都运行成功! 🎊${NC}"
    else
        echo -e "${YELLOW}有 $((total_count - success_count)) 个示例运行失败${NC}"
    fi
}

# 检查依赖
check_dependencies() {
    echo -e "${YELLOW}检查依赖...${NC}"
    
    # 检查 Rust 和 Cargo
    if command -v cargo &> /dev/null; then
        local rust_version=$(cargo --version)
        echo -e "${GREEN}✅ Cargo: $rust_version${NC}"
    else
        echo -e "${RED}❌ 未找到 Cargo，请安装 Rust${NC}"
        echo -e "${YELLOW}安装方法: https://rustup.rs/${NC}"
        exit 1
    fi
    
    # 检查项目是否可以编译
    echo -e "${YELLOW}检查项目编译...${NC}"
    if cargo check &> /dev/null; then
        echo -e "${GREEN}✅ 项目编译检查通过${NC}"
    else
        echo -e "${RED}❌ 项目编译检查失败${NC}"
        echo -e "${RED}错误信息:${NC}"
        cargo check
        echo -e "\n${YELLOW}请先解决编译错误再运行示例${NC}"
        exit 1
    fi
}

# 清理函数
clean_up() {
    echo -e "\n${YELLOW}清理临时文件...${NC}"
    
    # 清理可能的临时配置文件
    if [ -f "temp_config.toml" ]; then
        rm -f "temp_config.toml"
        echo -e "${GRAY}删除临时配置文件${NC}"
    fi
    
    # 可选：清理日志文件
    read -p "是否清理日志文件? (y/N): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if [ -d "logs" ]; then
            rm -rf "logs"
            echo -e "${GRAY}日志文件已清理${NC}"
        fi
    fi
}

# 显示帮助信息
show_help() {
    echo "用法: $0 [选项]"
    echo ""
    echo "选项:"
    echo "  all       运行所有示例"
    echo "  basic     运行基本使用示例"
    echo "  complete  运行完整示例集合"
    echo "  config    运行配置文件示例"
    echo "  clean     清理临时文件和日志"
    echo "  help      显示此帮助信息"
    echo ""
    echo "不带参数运行将显示交互式菜单"
}

# 信号处理
trap 'echo -e "\n${YELLOW}脚本被中断${NC}"; exit 130' INT
trap 'echo -e "\n${YELLOW}脚本被终止${NC}"; exit 143' TERM

# 主程序
main() {
    # 检查依赖
    check_dependencies
    
    # 如果有命令行参数，直接运行
    if [ $# -gt 0 ]; then
        case "$1" in
            "all")
                run_all_examples
                exit 0
                ;;
            "basic")
                IFS=':' read -r file name description <<< "${examples[0]}"
                run_example "$file" "$name" "$description"
                exit 0
                ;;
            "complete")
                IFS=':' read -r file name description <<< "${examples[1]}"
                run_example "$file" "$name" "$description"
                exit 0
                ;;
            "config")
                IFS=':' read -r file name description <<< "${examples[2]}"
                run_example "$file" "$name" "$description"
                exit 0
                ;;
            "clean")
                clean_up
                exit 0
                ;;
            "help"|"--help"|"-h")
                show_help
                exit 0
                ;;
            *)
                echo -e "${RED}未知参数: $1${NC}"
                echo -e "${YELLOW}可用参数: all, basic, complete, config, clean, help${NC}"
                echo -e "${YELLOW}使用 '$0 help' 查看详细帮助${NC}"
                exit 1
                ;;
        esac
    fi
    
    # 交互式菜单
    while true; do
        show_menu
        read -p "请输入选择: " choice
        
        case "$choice" in
            "0")
                run_all_examples
                ;;
            "1")
                IFS=':' read -r file name description <<< "${examples[0]}"
                run_example "$file" "$name" "$description"
                ;;
            "2")
                IFS=':' read -r file name description <<< "${examples[1]}"
                run_example "$file" "$name" "$description"
                ;;
            "3")
                IFS=':' read -r file name description <<< "${examples[2]}"
                run_example "$file" "$name" "$description"
                ;;
            "q"|"quit"|"exit")
                echo -e "\n${GREEN}再见! 👋${NC}"
                break
                ;;
            *)
                echo -e "${RED}无效选择，请重新输入${NC}"
                ;;
        esac
    done
}

# 执行主程序
main "$@"

# 可选清理
read -p "\n是否进行清理? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    clean_up
fi

echo -e "\n${GREEN}脚本执行完成${NC}"