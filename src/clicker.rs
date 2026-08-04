//! 点击引擎：后台线程按设定间隔执行鼠标点击与键盘按键。

use std::sync::Arc;
use std::time::Duration;

use crate::input::{self, MouseButton};
use crate::state::SharedState;

/// 空闲时的轮询间隔，与原版的 10ms 一致，用于降低 CPU 占用。
const IDLE_POLL: Duration = Duration::from_millis(10);

/// 最小点击间隔。
///
/// 间隔为 0 时循环将不带任何休眠地满速运行，会占满一个 CPU 核心并
/// 向系统输入队列灌入海量事件，可能导致界面卡死、难以用快捷键停止。
/// 这里强制下限保证 UI 与钩子线程始终有机会被调度。
const MIN_INTERVAL: Duration = Duration::from_millis(1);

/// 启动点击线程。
///
/// 循环内每轮都重新读取共享状态，因此运行时修改设置会立即生效。
pub(crate) fn spawn(state: Arc<SharedState>) {
    std::thread::spawn(move || loop {
        if !state.is_running() {
            std::thread::sleep(IDLE_POLL);
            continue;
        }

        if state.left() {
            input::click(MouseButton::Left);
        }
        if state.middle() {
            input::click(MouseButton::Middle);
        }
        if state.right() {
            input::click(MouseButton::Right);
        }

        let key = state.key_to_press();
        if !key.is_empty() {
            input::press_key_text(&key);
        }

        sleep_interruptible(&state, Duration::from_millis(state.interval_ms()));
    });
}

/// 分片休眠，期间若运行状态被取消则立即返回。
///
/// 若直接 sleep 整个间隔，大间隔（如 10 秒）下松开 F6 或按下停止后，
/// 仍会等到本轮休眠结束才真正停止，用户会感到"停不下来"。
fn sleep_interruptible(state: &SharedState, total: Duration) {
    let total = total.max(MIN_INTERVAL);
    let mut slept = Duration::ZERO;
    while slept < total {
        if !state.is_running() {
            return;
        }
        let step = IDLE_POLL.min(total - slept);
        std::thread::sleep(step);
        slept += step;
    }
}
