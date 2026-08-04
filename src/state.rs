//! 应用共享状态：在 UI 线程、点击线程与键盘钩子线程之间安全共享。

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// 切换操作的默认快捷键（F7）。
const DEFAULT_TOGGLE_KEY: u32 = 0x76;

/// 跨线程共享状态。
///
/// 点击线程每轮循环都会重新读取这些字段，因此运行过程中在界面上修改
/// 间隔、按键或切换快捷键会立即生效（与原 Python 版行为一致）。
#[derive(Debug)]
pub(crate) struct SharedState {
    running: AtomicBool,
    interval_ms: AtomicU64,
    left: AtomicBool,
    middle: AtomicBool,
    right: AtomicBool,
    /// 用于切换"运行/停止"的全局快捷键的虚拟键码（VK 码）。
    toggle_key: AtomicU32,
    /// 待发送的键盘按键文本，留空表示不发送。
    key_to_press: Mutex<String>,
}

impl Default for SharedState {
    fn default() -> Self {
        Self {
            running: AtomicBool::new(false),
            interval_ms: AtomicU64::new(100),
            left: AtomicBool::new(true),
            middle: AtomicBool::new(false),
            right: AtomicBool::new(false),
            toggle_key: AtomicU32::new(DEFAULT_TOGGLE_KEY),
            key_to_press: Mutex::new(String::new()),
        }
    }
}

impl SharedState {
    pub(crate) fn new_shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(crate) fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// 开始运行；返回 true 表示状态发生了变化。
    pub(crate) fn start(&self) -> bool {
        !self.running.swap(true, Ordering::Relaxed)
    }

    /// 停止运行；返回 true 表示状态发生了变化。
    pub(crate) fn stop(&self) -> bool {
        self.running.swap(false, Ordering::Relaxed)
    }

    /// 原子地翻转运行状态，返回翻转后的新状态。
    ///
    /// 使用 `fetch_xor` 而非 "读取-判断-写入"，避免多线程下的 TOCTOU 竞态
    /// （两次快速 F7 事件可能都读到 false 而重复调用 start）。
    pub(crate) fn toggle(&self) -> bool {
        !self.running.fetch_xor(true, Ordering::Relaxed)
    }

    pub(crate) fn interval_ms(&self) -> u64 {
        self.interval_ms.load(Ordering::Relaxed)
    }

    pub(crate) fn set_interval_ms(&self, v: u64) {
        self.interval_ms.store(v, Ordering::Relaxed);
    }

    pub(crate) fn left(&self) -> bool {
        self.left.load(Ordering::Relaxed)
    }

    pub(crate) fn set_left(&self, v: bool) {
        self.left.store(v, Ordering::Relaxed);
    }

    pub(crate) fn middle(&self) -> bool {
        self.middle.load(Ordering::Relaxed)
    }

    pub(crate) fn set_middle(&self, v: bool) {
        self.middle.store(v, Ordering::Relaxed);
    }

    pub(crate) fn right(&self) -> bool {
        self.right.load(Ordering::Relaxed)
    }

    pub(crate) fn set_right(&self, v: bool) {
        self.right.store(v, Ordering::Relaxed);
    }

    pub(crate) fn toggle_key(&self) -> u32 {
        self.toggle_key.load(Ordering::Relaxed)
    }

    /// 设置切换操作的全局快捷键。
    ///
    /// 返回 true 表示键码确实发生了变化。切换快捷键不会打断正在进行的点击，
    /// 仅改变下一次按键事件所匹配的目标。
    pub(crate) fn set_toggle_key(&self, vk: u32) -> bool {
        let prev = self.toggle_key.swap(vk, Ordering::Relaxed);
        prev != vk
    }

    /// 读取键盘按键文本。锁被投毒时退化为空字符串，避免 panic。
    pub(crate) fn key_to_press(&self) -> String {
        match self.key_to_press.lock() {
            Ok(g) => g.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub(crate) fn set_key_to_press(&self, v: &str) {
        match self.key_to_press.lock() {
            Ok(mut g) => v.clone_into(&mut g),
            Err(poisoned) => v.clone_into(&mut poisoned.into_inner()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_stopped_with_default_toggle_key() {
        let s = SharedState::default();
        assert!(!s.is_running());
        assert_eq!(s.toggle_key(), DEFAULT_TOGGLE_KEY);
    }

    #[test]
    fn start_and_stop_report_state_change() {
        let s = SharedState::default();
        assert!(s.start(), "首次启动应报告状态变化");
        assert!(!s.start(), "重复启动不应报告变化");
        assert!(s.stop(), "首次停止应报告状态变化");
        assert!(!s.stop(), "重复停止不应报告变化");
    }

    #[test]
    fn toggle_flips_and_returns_new_state() {
        let s = SharedState::default();
        assert!(s.toggle(), "首次 toggle 应返回运行中");
        assert!(s.is_running());
        assert!(!s.toggle(), "再次 toggle 应返回已停止");
        assert!(!s.is_running());
    }

    /// 切换快捷键设置：运行中修改快捷键不应打断点击，且确实更新键码。
    #[test]
    fn switching_toggle_key_while_running_keeps_running() {
        let s = SharedState::default();
        s.toggle();
        assert!(s.is_running());

        assert!(s.set_toggle_key(0x70)); // F1
        assert_eq!(s.toggle_key(), 0x70);
        assert!(s.is_running(), "修改切换快捷键不应打断运行");
    }

    /// 设置为相同快捷键时不应报告变化。
    #[test]
    fn setting_same_toggle_key_is_noop() {
        let s = SharedState::default();
        let vk = s.toggle_key();
        assert!(!s.set_toggle_key(vk), "相同键码不应报告变化");
    }

    #[test]
    fn key_text_round_trips() {
        let s = SharedState::default();
        assert_eq!(s.key_to_press(), "");
        s.set_key_to_press("a");
        assert_eq!(s.key_to_press(), "a");
    }
}
