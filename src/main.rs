//! 连点器：原 Python/Tkinter 版本的 Rust 重构实现。
//!
//! 保留全部原有功能：可配置点击间隔、左/中/右键任意组合、可选键盘按键、
//! 可由用户自定义绑定为任意按键的全局切换快捷键，以及实时设置信息面板。

// 与原 .pyw 一致：运行时不弹出控制台窗口。
#![windows_subsystem = "windows"]

mod clicker;
mod hotkey;
mod input;
mod state;
mod ui;

use std::sync::Arc;

use eframe::egui;

use crate::state::SharedState;
use crate::ui::{AutoClickerApp, WINDOW_MIN_HEIGHT, WINDOW_MIN_WIDTH};

fn main() -> eframe::Result<()> {
    let state = SharedState::new_shared();

    clicker::spawn(Arc::clone(&state));
    // 键盘钩子（Raw Input）需在本程序窗口的 hwnd 可用后安装，
    // 因此在 AutoClickerApp 首帧通过 eframe 的 Frame 取得 hwnd 时调用 hotkey::install。

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(egui::vec2(WINDOW_MIN_WIDTH, WINDOW_MIN_HEIGHT))
            .with_min_inner_size(egui::vec2(WINDOW_MIN_WIDTH, WINDOW_MIN_HEIGHT))
            .with_resizable(true)
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
                    .unwrap_or_default(),
            ),
        ..Default::default()
    };

    eframe::run_native(
        "连点器",
        options,
        Box::new(move |cc| Ok(Box::new(AutoClickerApp::new(cc, state)))),
    )
}
