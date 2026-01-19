# NanoTrans

NanoTrans 是一款跨平台的极致轻量化输入翻译助手，支持 Windows 和 macOS。它深度集成于系统底层，通过锁定输入光标位置提供即时的文本翻译与原地回填功能。

#  项目仍在开发!!! 现版本不推荐试用哦~
当前版本：0.1.0-alpha.2

## 下载

- Windows (exe): https://github.com/qqqqqf-q/NanoTrans/releases/download/latest/nanotrans.exe
- macOS (app): https://github.com/qqqqqf-q/NanoTrans/releases/download/latest/NanoTrans-macOS.app.zip

## 平台支持

- ✅ **Windows**: 完整功能支持，精确的文本光标位置检测
- ✅ **macOS**: 核心功能支持（需要辅助功能权限）

## 项目功能

* 1.在输入框输入一段文本  
* 2.Ctrl+A  
* 3.Alt+Q(翻译快捷键)  
* 4.Ctrl+V  
* 5.你就获得了一段翻译至另一个语言的文本

## 核心功能

1. 输入光标跟随：
   - Windows: 利用 GetGUIThreadInfo API 锁定文本输入框的光标位置
   - macOS: 使用鼠标位置作为替代方案
   - 确保翻译浮窗精准显示在输入视线内
2. 原地转译与回填：通过全局热键触发，自动完成 选中文本 -> 复制 -> 翻译 -> 结果展示 -> 自动粘贴 的全流程，实现输入内容的快速替换。
3. 极致轻量化：弃用浏览器内核，采用原生绘图引擎，确保程序启动即达、内存占用极低、磁盘占用小于 3MB。
4. 常驻系统托盘：支持后台静默运行，通过托盘菜单进行配置管理，不干扰正常的桌面工作流。
5. 跨平台支持：统一的代码库，支持 Windows 和 macOS 双平台。

## 技术方案

1. 核心语言：Rust。
2. UI 框架：Slint。采用声明式语法并编译为原生机器码，提供高效的 GPU 渲染。
3. 系统接口：
   - Windows: windows-rs，直接调用 Win32 API
   - macOS: cocoa, core-graphics, core-foundation
4. 剪贴板管理：arboard。实现跨平台的剪贴板读写。
5. 网络通信：reqwest (搭配 rustls-tls)。实现轻量级的异步 API 请求。
6. 跨平台架构：模块化设计，平台特定代码封装在独立模块中。

## 使用说明

1. 配置：右键点击系统托盘图标进入设置页面，填入您的翻译服务 API 密钥。
2. 触发：在任意文本编辑器或输入框中选中需要翻译的内容。
3. 转换：按下预设快捷键（默认 Ctrl+Shift+T / macOS 上为 Cmd+Shift+T）。
4. 替换：翻译结果将出现在光标处，确认无误后点击或按回车键，原文将被自动替换为译文。

### macOS 特别说明

首次运行时需要授予辅助功能权限：
1. 打开"系统偏好设置" > "安全性与隐私" > "辅助功能"
2. 添加 NanoTrans 到允许列表
3. 重启应用程序

若下载的 .app 无法打开，可执行：
`xattr -dr com.apple.quarantine NanoTrans.app`

## 构建与开发

若要自行编译并实现极致体积优化，请确保已安装 Rust 开发环境。

```bash
# 获取源码
git clone https://github.com/yourname/NanoTrans.git
cd NanoTrans

# 编译发布版本
# 该版本已配置 LTO (Link Time Optimization) 与 Strip 优化
cargo build --release

# Windows: 可选安装 WiX Toolset 用于生成 MSI 安装包
# macOS: 编译后的二进制位于 target/release/nanotrans
```

macOS 打包 .app：

```bash
./scripts/build-macos-app.sh
```

## 体积优化说明

本项目在 Cargo.toml 中进行了以下优化配置以确保体积压缩：

* 优化级别设置为 z (opt-level = "z") 以优化二进制大小。
* 开启全程序链接优化 (lto = true)。
* 移除所有调试符号 (strip = true)。
* 减少并行编译单元以提升优化空间。

## 许可协议

本项目基于 MIT 协议开源。
