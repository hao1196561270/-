# 连点器 auto-clicker

一个用 Rust 编写的 Windows 鼠标/键盘自动连点器，基于 `eframe` + `egui` 图形界面，使用 Windows Raw Input 实现全局按键监听。

## 功能特性

- 可配置点击间隔（毫秒）与点击次数（0 表示无限）。
- 可切换点击鼠标左键 / 右键。
- 支持自定义"切换按键"：用于在窗口聚焦或未聚焦状态下启动 / 停止连点。默认按键为 `F7`，可在界面中重新绑定到任意键。
- 全局热键通过 Raw Input 实现，无论连点器窗口是否处于前台焦点，都能正确捕获按键，避免传统低级钩子在窗口聚焦时失效的问题。

## 使用方法

1. 下载 Release 中的 `auto_clicker.exe` 直接运行（Windows 64 位）。
2. 或自行编译：

```bash
cargo build --release
# 编译产物：target/release/auto_clicker.exe
```

3. 在界面中设置点击间隔、点击次数、鼠标按键与切换按键。
4. 点击"开始"或按下切换按键即可启动 / 停止连点。

## 编译说明

- 需要 Windows SDK 中的 `rc.exe` 用于嵌入图标资源（由 `build.rs` 自动调用 `winresource`）。
- 依赖 `windows` crate 访问 Windows API，包括 `Raw Input`、`SendInput`、`RegisterRawInputDevices` 等。

## 目录结构

- `src/main.rs`：程序入口，构建窗口与共享状态。
- `src/clicker.rs`：连点后台线程，按配置发送输入。
- `src/hotkey.rs`：基于 Raw Input 的全局键盘监听与切换键捕获。
- `src/state.rs`：共享配置状态（Arc + 原子类型）。
- `src/ui.rs`：egui 界面与交互逻辑。
- `src/input.rs`：输入发送封装。

## 许可

本项目仅供学习与个人使用。
