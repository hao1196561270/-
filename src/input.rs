//! 输入合成：通过 Win32 `SendInput` 发送鼠标点击与键盘按键。

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, VkKeyScanW, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEINPUT, MOUSE_EVENT_FLAGS, VIRTUAL_KEY, VK_SHIFT,
};

/// 鼠标按键类型。
#[derive(Clone, Copy)]
pub(crate) enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseButton {
    fn flags(self) -> (MOUSE_EVENT_FLAGS, MOUSE_EVENT_FLAGS) {
        match self {
            Self::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            Self::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
            Self::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        }
    }
}

fn mouse_input(flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn key_input(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// 发送一批输入事件。
fn send(inputs: &[INPUT]) {
    // SAFETY: `inputs` 是有效切片，cbSize 传入 INPUT 的真实大小，
    // 符合 SendInput 的调用契约。
    let sent = unsafe { SendInput(inputs, core::mem::size_of::<INPUT>() as i32) };
    debug_assert_eq!(sent as usize, inputs.len(), "SendInput 未完整发送事件");
}

/// 执行一次完整的鼠标点击（按下 + 抬起）。
pub(crate) fn click(button: MouseButton) {
    let (down, up) = button.flags();
    send(&[mouse_input(down), mouse_input(up)]);
}

/// 按下并抬起给定文本的第一个字符。
///
/// 优先通过 `VkKeyScanW` 取得虚拟键码（必要时附带 Shift），
/// 若该字符在当前键盘布局中不存在，则退化为 Unicode 直发。
pub(crate) fn press_key_text(text: &str) {
    let Some(ch) = text.chars().next() else {
        return;
    };

    let mut utf16 = [0u16; 2];
    let encoded = ch.encode_utf16(&mut utf16);
    // 非基本平面字符无法用单个虚拟键表示，直接走 Unicode 分支。
    let single = if encoded.len() == 1 { encoded[0] } else { 0 };

    // SAFETY: VkKeyScanW 仅读取传入的字符值，无额外前置条件。
    let scan_result = if single != 0 {
        unsafe { VkKeyScanW(single) }
    } else {
        -1
    };

    if scan_result != -1 {
        let vk = (scan_result & 0xFF) as u16;
        let shift_state = (scan_result >> 8) & 0xFF;
        let needs_shift = shift_state & 0x01 != 0;

        let mut inputs = Vec::with_capacity(4);
        if needs_shift {
            inputs.push(key_input(VK_SHIFT.0, 0, KEYBD_EVENT_FLAGS(0)));
        }
        inputs.push(key_input(vk, 0, KEYBD_EVENT_FLAGS(0)));
        inputs.push(key_input(vk, 0, KEYEVENTF_KEYUP));
        if needs_shift {
            inputs.push(key_input(VK_SHIFT.0, 0, KEYEVENTF_KEYUP));
        }
        send(&inputs);
        return;
    }

    // 回退：按 Unicode 码元逐个发送。
    let mut inputs = Vec::with_capacity(encoded.len() * 2);
    for unit in encoded.iter().copied() {
        inputs.push(key_input(0, unit, KEYEVENTF_UNICODE));
        inputs.push(key_input(0, unit, KEYEVENTF_UNICODE | KEYEVENTF_KEYUP));
    }
    send(&inputs);
}
