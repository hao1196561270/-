//! 界面层：复刻原 Tkinter 窗口的布局、控件与信息面板。

use std::sync::Arc;

use eframe::egui;

use crate::hotkey;
use crate::state::SharedState;

/// 窗口尺寸（物理像素），用于首帧设置。
///
/// 这些值对应屏幕上的实际显示像素。egui 内部使用逻辑点，
/// 首帧会根据 pixels_per_point 自动换算。
pub(crate) const WINDOW_MIN_WIDTH: f32 = 320.0;
pub(crate) const WINDOW_MIN_HEIGHT: f32 = 560.0;

/// 候选中文字体，按优先级尝试加载，保证中文不显示为方块。
///
/// 本项目未启用 eframe 的 `default_fonts`（内置字体约占 1MB 且不含中文），
/// 因此系统字体是唯一字体来源，候选列表需覆盖各版本 Windows 的常见字体。
const CJK_FONTS: [&str; 7] = [
    r"C:\Windows\Fonts\msyh.ttc",    // 微软雅黑
    r"C:\Windows\Fonts\msyh.ttf",    // 微软雅黑（旧版）
    r"C:\Windows\Fonts\simhei.ttf",  // 黑体
    r"C:\Windows\Fonts\Deng.ttf",    // 等线
    r"C:\Windows\Fonts\simsun.ttc",  // 宋体
    r"C:\Windows\Fonts\msjh.ttc",    // 微软正黑（繁体环境）
    r"C:\Windows\Fonts\segoeui.ttf", // 兜底：无中文但保证有字形
];

/// 安装系统字体。
///
/// 逐个尝试候选路径，成功即返回。全部失败时保留 egui 的空字体定义，
/// 程序仍可运行（仅文本不可见），不会崩溃。
fn install_cjk_font(ctx: &egui::Context) {
    for path in CJK_FONTS {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };

        let mut fonts = egui::FontDefinitions::empty();
        fonts
            .font_data
            .insert("sys".to_owned(), egui::FontData::from_owned(bytes));
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("sys".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("sys".to_owned());
        ctx.set_fonts(fonts);
        return;
    }
}

/// 水平居中放置一行控件。
///
/// egui 的 `vertical_centered` 只居中单个控件，横向多控件需要先测量再回退绘制，
/// 这里用两遍布局实现：首帧按左对齐测得宽度，之后按测得宽度计算左侧偏移。
fn centered_row<R>(ui: &mut egui::Ui, id: egui::Id, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let measured: Option<f32> = ui.data(|d| d.get_temp(id));
    let offset = measured
        .map(|w| ((ui.available_width() - w) * 0.5).max(0.0))
        .unwrap_or(0.0);

    let mut width = 0.0;
    let result = ui
        .horizontal(|ui| {
            ui.add_space(offset);
            let start = ui.cursor().min.x;
            let r = add(ui);
            width = ui.cursor().min.x - start;
            r
        })
        .inner;

    ui.data_mut(|d| d.insert_temp(id, width));
    result
}

pub(crate) struct AutoClickerApp {
    state: Arc<SharedState>,
    /// 间隔输入框的文本缓冲，允许用户临时输入非法值而不丢失焦点。
    interval_text: String,
    key_text: String,
    /// 是否处于"等待用户按下新切换键"的监听态。
    rebinding: bool,
    /// 是否已完成首帧的窗口尺寸校正。
    sized: bool,
}

impl AutoClickerApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>, state: Arc<SharedState>) -> Self {
        install_cjk_font(&cc.egui_ctx);

        let interval_text = state.interval_ms().to_string();
        let key_text = state.key_to_press();
        Self {
            state,
            interval_text,
            key_text,
            rebinding: false,
            sized: false,
        }
    }

    /// 生成信息面板文本，对应原版的 `update_info`。
    fn info_text(&self) -> String {
        let interval = self.state.interval_ms();
        let mut buttons = String::new();
        if self.state.left() {
            buttons.push('左');
        }
        if self.state.middle() {
            buttons.push('中');
        }
        if self.state.right() {
            buttons.push('右');
        }

        let key = self.state.key_to_press();
        let key_display = if key.is_empty() { "无" } else { key.as_str() };

        let toggle_name = hotkey::vk_name(self.state.toggle_key());
        let status = if self.state.is_running() {
            "运行中"
        } else {
            "停止"
        };

        format!(
            "当前设置:\n间隔: {} 毫秒 ({:.3} 秒)\n鼠标按键: {}\n键盘按键: {}\n切换按键: {}\n状态: {}",
            interval,
            interval as f64 / 1000.0,
            buttons,
            key_display,
            toggle_name,
            status,
        )
    }
}

impl eframe::App for AutoClickerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 首帧校正窗口尺寸：此时 viewport 已就绪，且能拿到真实 DPI 缩放系数。
        // 使用最小尺寸确保所有内容在默认视图中完整可见，无需滚动。
        if !self.sized {
            self.sized = true;
            let scale = ctx.pixels_per_point().max(0.1);
            let logical = egui::vec2(WINDOW_MIN_WIDTH / scale, WINDOW_MIN_HEIGHT / scale);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(logical));
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(logical));

            // 安装 Raw Input 监听（覆盖聚焦/未聚焦两种状态）。
            // 通过固定窗口标题查找 HWND，无需依赖 eframe 的内部窗口句柄 API。
            hotkey::install(Arc::clone(&self.state));
        }

        // 快捷键在后台线程改变状态后，需要主动刷新界面。
        if hotkey::take_repaint_request() {
            ctx.request_repaint();
        }
        // 监听新切换键期间，持续轮询捕获下一次按键事件。
        if self.rebinding {
            if let Some(new_vk) = hotkey::take_binding() {
                self.state.set_toggle_key(new_vk);
                self.rebinding = false;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
        }
        // 运行状态下持续刷新，让信息面板的状态保持实时。
        ctx.request_repaint_after(std::time::Duration::from_millis(100));

        egui::CentralPanel::default().show(ctx, |ui| {
            // 使用紧凑垂直布局，减少不必要的内边距，确保内容在窗口内完整显示。
            ui.vertical_centered(|ui| {
                ui.add_space(4.0);
                ui.label("点击间隔(毫秒):");
                centered_row(ui, egui::Id::new("row_interval"), |ui| {
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.interval_text).desired_width(70.0),
                    );
                    if resp.changed() {
                        if let Ok(v) = self.interval_text.trim().parse::<u64>() {
                            self.state.set_interval_ms(v);
                        }
                    }
                    ui.label("ms");
                });

                ui.add_space(4.0);
                ui.label("鼠标按键:");
                centered_row(ui, egui::Id::new("row_buttons"), |ui| {
                    let mut left = self.state.left();
                    if ui.checkbox(&mut left, "左键").changed() {
                        self.state.set_left(left);
                    }
                    let mut middle = self.state.middle();
                    if ui.checkbox(&mut middle, "中键").changed() {
                        self.state.set_middle(middle);
                    }
                    let mut right = self.state.right();
                    if ui.checkbox(&mut right, "右键").changed() {
                        self.state.set_right(right);
                    }
                });

                ui.add_space(4.0);
                ui.label("键盘按键(留空则不按):");
                let resp =
                    ui.add(egui::TextEdit::singleline(&mut self.key_text).desired_width(150.0));
                if resp.changed() {
                    self.state.set_key_to_press(&self.key_text);
                }

                ui.add_space(4.0);
                ui.label("切换按键(用于启动/停止):");
                centered_row(ui, egui::Id::new("row_toggle_key"), |ui| {
                    if self.rebinding {
                        ui.label(egui::RichText::new("按下任意键...").color(egui::Color32::YELLOW));
                        if ui.button("取消").clicked() {
                            hotkey::cancel_listen();
                            self.rebinding = false;
                        }
                    } else {
                        let name = hotkey::vk_name(self.state.toggle_key());
                        ui.label(format!("当前: {}", name));
                        if ui.button("重新绑定").clicked() {
                            self.rebinding = true;
                            hotkey::begin_listen();
                        }
                    }
                });

                ui.add_space(6.0);
                let running = self.state.is_running();
                if running {
                    ui.colored_label(egui::Color32::from_rgb(0, 150, 0), "状态: 运行中");
                } else {
                    ui.colored_label(egui::Color32::RED, "状态: 停止");
                }

                ui.add_space(3.0);
                if ui
                    .add_enabled(!running, egui::Button::new("启动"))
                    .clicked()
                {
                    self.state.start();
                }
                ui.add_space(2.0);
                if ui.add_enabled(running, egui::Button::new("停止")).clicked() {
                    self.state.stop();
                }

                ui.add_space(6.0);
                let mut info = self.info_text();
                ui.add(
                    egui::TextEdit::multiline(&mut info)
                        .desired_width(f32::INFINITY)
                        .desired_rows(7)
                        .interactive(false),
                );
            });
        });
    }
}
