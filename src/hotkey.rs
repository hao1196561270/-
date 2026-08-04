//! 全局快捷键：基于 Raw Input（原始输入）监听用户自定义的切换键。
//!
//! 早期方案先后尝试过 `WH_KEYBOARD_LL` 与 `WH_GETMESSAGE`，但都有焦点盲区：
//! - `WH_KEYBOARD_LL` 装在泵消息线程时，当本程序窗口是前台焦点，系统把键盘输入
//!   直接投递给窗口过程，低级钩子回调不再被调用；
//! - `WH_GETMESSAGE` 需要精确命中 eframe/winit 的消息泵线程，而该线程并非调用
//!   `spawn` 的主线程，导致钩子从不触发。
//!
//! 最终采用 Raw Input：向本程序窗口的 `hwnd` 注册键盘原始输入（默认标志，
//! 即窗口即使不是前台也能收到输入），并通过子类化 `WndProc` 拦截 `WM_INPUT`。
//! 这样无论窗口是否聚焦，所有按键都会以 `WM_INPUT` 形式送达我们的窗口过程，
//! 从而稳定捕获。回调始终链式调用原 `WndProc`，不吞掉按键，也不影响 egui 输入。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::{
    GetRawInputData, RAWINPUT, RAWINPUTDEVICE, RAWINPUTDEVICE_FLAGS, RAWINPUTHEADER,
    RegisterRawInputDevices, HRAWINPUT, RID_INPUT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, FindWindowW, GWLP_WNDPROC, SetWindowLongPtrW, WM_INPUT,
};

use crate::state::SharedState;

/// 钩子回调访问共享状态的全局入口。
static HOOK_STATE: OnceLock<Arc<SharedState>> = OnceLock::new();

/// 通知 UI 需要重绘。
static NEEDS_REPAINT: AtomicBool = AtomicBool::new(false);

/// 是否处于"等待用户按下新切换键"的监听态。
static LISTENING: AtomicBool = AtomicBool::new(false);

/// 监听态捕获到的按键虚拟键码，捕获后由 UI 线程取走并清空。
static BIND_BUFFER: Mutex<Option<u32>> = Mutex::new(None);

/// 被子类化替换前的原始窗口过程，用于链式调用。
static OLD_WNDPROC: Mutex<Option<usize>> = Mutex::new(None);

/// 快捷键是否改变了状态，供 UI 线程轮询后重绘。
pub(crate) fn take_repaint_request() -> bool {
    NEEDS_REPAINT.swap(false, Ordering::Relaxed)
}

/// 进入监听态，等待下一次按键。若已在区间内则忽略。
pub(crate) fn begin_listen() -> bool {
    if let Ok(mut g) = BIND_BUFFER.lock() {
        *g = None; // 丢弃上一次未取走的残留。
    }
    LISTENING.swap(true, Ordering::Relaxed)
}

/// 取走监听态捕获的按键码（若已捕获）。取走即结束监听态。
pub(crate) fn take_binding() -> Option<u32> {
    let taken = if let Ok(mut g) = BIND_BUFFER.lock() {
        g.take()
    } else {
        None
    };
    if taken.is_some() {
        LISTENING.store(false, Ordering::Relaxed);
    }
    taken
}

/// 取消监听态。
pub(crate) fn cancel_listen() {
    LISTENING.store(false, Ordering::Relaxed);
    if let Ok(mut g) = BIND_BUFFER.lock() {
        *g = None;
    }
}

/// 请求一次界面重绘（按键事件导致状态变化后调用）。
fn request_repaint() {
    NEEDS_REPAINT.store(true, Ordering::Relaxed);
}

/// 统一的按键捕获逻辑：供窗口过程共用。
///
/// 监听态下把按下事件记录为新的切换键（不触发切换）；否则若等于当前切换键
/// 则翻转运行状态。幂等，重复触发不会产生异常。
fn capture_key(vk: u32) {
    if vk == 0 || vk == 0xFF {
        // 0 与 VK_PACKET(0xFF) 不是有效绑定目标，忽略。
        return;
    }
    if LISTENING.load(Ordering::Relaxed) {
        if let Ok(mut g) = BIND_BUFFER.lock() {
            *g = Some(vk);
        }
        LISTENING.store(false, Ordering::Relaxed);
        request_repaint();
    } else if let Some(state) = HOOK_STATE.get() {
        if vk == state.toggle_key() {
            state.toggle();
            request_repaint();
        }
    }
}

/// 子类化的窗口过程：拦截 `WM_INPUT` 提取键盘虚拟键码。
///
/// SAFETY: 仅在 `install` 中对有效的本程序窗口子类化后调用；除 `WM_INPUT` 外
/// 一律透传给原窗口过程，保证 egui 的键盘输入不受影响。
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_INPUT {
        // 首次调用取缓冲区大小。
        let mut size: u32 = 0;
        // SAFETY: pcbSize 为有效可写指针，其余为 null/0 仅查询大小。
        let _ = GetRawInputData(
            HRAWINPUT(lparam.0 as *mut std::ffi::c_void),
            RID_INPUT,
            None,
            &mut size,
            std::mem::size_of::<RAWINPUTHEADER>() as u32,
        );
        if size > 0 {
            let mut buf: Vec<u8> = vec![0u8; size as usize];
            // SAFETY: buf 大小足够容纳原始输入数据。
            let got = GetRawInputData(
                HRAWINPUT(lparam.0 as *mut std::ffi::c_void),
                RID_INPUT,
                Some(buf.as_mut_ptr() as *mut std::ffi::c_void),
                &mut size,
                std::mem::size_of::<RAWINPUTHEADER>() as u32,
            );
            if got != u32::MAX {
                // SAFETY: 缓冲区已被填充为有效的 RAWINPUT。
                let ri = &*(buf.as_ptr() as *const RAWINPUT);
                // dwType == 1 (RIM_TYPEKEYBOARD) 表示键盘输入。
                if ri.header.dwType == 1 {
                    // SAFETY: 联合体读取，此处已知为键盘数据。
                    let kb = unsafe { ri.data.keyboard };
                    // Flags 的低位：0=按下(MAKE)，1=抬起(BREAK)。
                    let is_down = (kb.Flags & 0x01) == 0;
                    if is_down {
                        capture_key(kb.VKey as u32);
                    }
                }
            }
        }
    }

    // 链式调用原窗口过程。
    // SAFETY: OLD_WNDPROC 保存的是有效的原始窗口过程指针。
    let old = OLD_WNDPROC.lock().ok().and_then(|g| *g);
    if let Some(old) = old {
        // SAFETY: 透传原始参数给原窗口过程。
        let old_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT =
            unsafe { std::mem::transmute(old) };
        unsafe { CallWindowProcW(Some(old_proc), hwnd, msg, wparam, lparam) }
    } else {
        // 无原始过程时（不应发生），返回 0。
        LRESULT(0)
    }
}

/// 安装原始输入监听并子类化窗口过程。
///
/// 通过固定窗口标题 "连点器" 用 `FindWindowW` 取得本程序窗口的 `hwnd`
/// （eframe 在首帧时窗口已创建）。注册键盘原始输入（默认标志：即使窗口未聚焦
/// 也能收到输入），并把窗口过程替换为 `wnd_proc` 以拦截 `WM_INPUT`。
pub(crate) fn install(state: Arc<SharedState>) {
    // 忽略重复安装。
    if HOOK_STATE.set(state).is_err() {
        return;
    }

    // SAFETY: 按窗口标题查找本程序窗口；返回有效 HWND 或错误。
    let hwnd = match unsafe { FindWindowW(None, w!("连点器")) } {
        Ok(h) if !h.is_invalid() => h,
        _ => return,
    };

    // 注册键盘原始输入到本窗口。
    // SAFETY: device 为有效的 RAWINPUTDEVICE 数组，hwndTarget 指向本程序窗口。
    let device = RAWINPUTDEVICE {
        usUsagePage: 0x01, // HID_USAGE_PAGE_GENERIC
        usUsage: 0x06,     // HID_USAGE_GENERIC_KEYBOARD
        dwFlags: RAWINPUTDEVICE_FLAGS(0), // 默认：窗口即使非前台也能收到输入
        hwndTarget: hwnd,
    };
    // SAFETY: 指向有效的 RAWINPUTDEVICE 切片；该函数返回 Result，失败即返回。
    if unsafe { RegisterRawInputDevices(&[device], std::mem::size_of::<RAWINPUTDEVICE>() as u32) }
        .is_err()
    {
        return;
    }

    // 子类化窗口过程：保存原过程，替换为我们的 wnd_proc。
    // SAFETY: hwnd 有效；获取并设置的都是 GWLP_WNDPROC 槽位的函数指针。
    let old = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, wnd_proc as *const () as isize) };
    if old != 0 {
        if let Ok(mut g) = OLD_WNDPROC.lock() {
            *g = Some(old as usize);
        }
    }
}

/// 将虚拟键码转换为对用户友好的名称。
///
/// 覆盖连点器常见可绑定键：功能键、字母数字、修饰键与少数控制键。
/// 未覆盖的键码回退为 `VK(0xNN)` 形式，保证始终有可读文本。
pub(crate) fn vk_name(vk: u32) -> String {
    let name: String = match vk {
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x10 => "Shift".into(),
        0x11 => "Ctrl".into(),
        0x12 => "Alt".into(),
        0x13 => "Pause".into(),
        0x14 => "CapsLock".into(),
        0x1B => "Esc".into(),
        0x20 => "Space".into(),
        0x21 => "PageUp".into(),
        0x22 => "PageDown".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x2D => "Insert".into(),
        0x2E => "Delete".into(),
        0x30..=0x39 => ((b'0' + (vk - 0x30) as u8) as char).to_string(),
        0x41..=0x5A => ((b'A' + (vk - 0x41) as u8) as char).to_string(),
        0x70..=0x87 => return format!("F{}", vk - 0x6F),
        0x90 => "NumLock".into(),
        0x91 => "ScrollLock".into(),
        _ => return format!("VK(0x{vk:02X})"),
    };
    name
}
