use crate::config::{DEFAULT_PROXY, ProxySetting};
use std::mem::zeroed;
use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::*,
    UI::{Controls::*, HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

const WINDOW_WIDTH: i32 = 580;
const WINDOW_HEIGHT: i32 = 290;
const ID_SAVE: u16 = 1001;
const ID_CANCEL: u16 = 1002;
const ID_DIRECT: u16 = 1003;
const ID_DEFAULT: u16 = 1004;

struct State {
    edit: HWND,
    direct: HWND,
    direct_checked: bool,
    dpi: u32,
    result: Option<ProxySetting>,
    finished: bool,
}

pub fn shift_pressed() -> bool {
    unsafe { GetAsyncKeyState(VK_SHIFT.into()) < 0 }
}

pub fn show(current: &ProxySetting) -> Result<Option<ProxySetting>, String> {
    unsafe {
        SetProcessDPIAware();
        let dpi = GetDpiForSystem().max(96);
        let window_width = scale(WINDOW_WIDTH, dpi);
        let window_height = scale(WINDOW_HEIGHT, dpi);
        let instance = GetModuleHandleW(null());
        let class_name = wide("StartChatGPTSettings");
        let class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hIcon: LoadImageW(
                instance,
                std::ptr::without_provenance(1),
                IMAGE_ICON,
                scale(32, dpi),
                scale(32, dpi),
                LR_SHARED,
            ),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: std::ptr::without_provenance_mut((COLOR_BTNFACE + 1) as usize),
            lpszClassName: class_name.as_ptr(),
            ..zeroed()
        };
        RegisterClassW(&class);

        let mut state = State {
            edit: null_mut(),
            direct: null_mut(),
            direct_checked: matches!(current, ProxySetting::Direct),
            dpi,
            result: None,
            finished: false,
        };
        let x = (GetSystemMetrics(0) - window_width) / 2;
        let y = (GetSystemMetrics(1) - window_height) / 2;
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            class_name.as_ptr(),
            wide("startChatGPT 代理设置").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            x,
            y,
            window_width,
            window_height,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );
        if hwnd.is_null() {
            return Err("无法创建代理设置窗口".into());
        }
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&mut state as *mut State) as isize);

        let font = CreateFontW(
            -scale(14, dpi),
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Microsoft YaHei UI").as_ptr(),
        );

        create_control(
            hwnd,
            instance,
            "STATIC",
            "代理服务器地址",
            scale(30, dpi),
            scale(28, dpi),
            scale(500, dpi),
            scale(24, dpi),
            0,
            0,
            font,
        );
        state.edit = create_control(
            hwnd,
            instance,
            "EDIT",
            current.proxy_url().unwrap_or(DEFAULT_PROXY),
            scale(30, dpi),
            scale(63, dpi),
            scale(510, dpi),
            scale(26, dpi),
            WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL as u32,
            0,
            font,
        );
        create_control(
            hwnd,
            instance,
            "STATIC",
            "支持 http、https、socks4 和 socks5，例如：http://127.0.0.1:10808",
            scale(30, dpi),
            scale(97, dpi),
            scale(510, dpi),
            scale(24, dpi),
            0,
            0,
            font,
        );
        state.direct = create_control(
            hwnd,
            instance,
            "BUTTON",
            "不使用代理（直接连接）",
            scale(30, dpi),
            scale(130, dpi),
            scale(250, dpi),
            scale(30, dpi),
            WS_TABSTOP | BS_OWNERDRAW as u32,
            ID_DIRECT,
            font,
        );
        create_control(
            hwnd,
            instance,
            "BUTTON",
            "恢复默认",
            scale(30, dpi),
            scale(190, dpi),
            scale(95, dpi),
            scale(32, dpi),
            WS_TABSTOP | BS_PUSHBUTTON as u32,
            ID_DEFAULT,
            font,
        );
        create_control(
            hwnd,
            instance,
            "BUTTON",
            "取消",
            scale(350, dpi),
            scale(190, dpi),
            scale(80, dpi),
            scale(32, dpi),
            WS_TABSTOP | BS_PUSHBUTTON as u32,
            ID_CANCEL,
            font,
        );
        create_control(
            hwnd,
            instance,
            "BUTTON",
            "保存并启动",
            scale(440, dpi),
            scale(190, dpi),
            scale(100, dpi),
            scale(32, dpi),
            WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            ID_SAVE,
            font,
        );

        if matches!(current, ProxySetting::Direct) {
            EnableWindow(state.edit, 0);
        }

        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        if !matches!(current, ProxySetting::Direct) {
            SetFocus(state.edit);
        }

        let mut message: MSG = zeroed();
        while !state.finished && GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        DeleteObject(font);
        Ok(state.result)
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn create_control(
    parent: HWND,
    instance: HINSTANCE,
    class: &str,
    text: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    style: u32,
    id: u16,
    font: HFONT,
) -> HWND {
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            width,
            height,
            parent,
            std::ptr::without_provenance_mut(id as usize),
            instance,
            null_mut(),
        )
    };
    unsafe { SendMessageW(hwnd, WM_SETFONT, font as WPARAM, 1) };
    hwnd
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    match message {
        WM_COMMAND => {
            unsafe { handle_command(hwnd, (w_param & 0xffff) as u16) };
            0
        }
        WM_DRAWITEM => unsafe { draw_direct_checkbox(hwnd, l_param) },
        WM_CLOSE => {
            unsafe {
                if let Some(state) = state(hwnd) {
                    state.finished = true;
                }
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, message, w_param, l_param) },
    }
}

unsafe fn handle_command(hwnd: HWND, id: u16) {
    let Some(state) = (unsafe { state(hwnd) }) else {
        return;
    };
    match id {
        ID_DIRECT => {
            state.direct_checked = !state.direct_checked;
            unsafe {
                EnableWindow(state.edit, (!state.direct_checked).into());
                InvalidateRect(state.direct, null(), 1);
            }
            if !state.direct_checked {
                unsafe { SetFocus(state.edit) };
            }
        }
        ID_DEFAULT => unsafe {
            SetWindowTextW(state.edit, wide(DEFAULT_PROXY).as_ptr());
            state.direct_checked = false;
            InvalidateRect(state.direct, null(), 1);
            EnableWindow(state.edit, 1);
            SetFocus(state.edit);
        },
        ID_CANCEL => unsafe {
            state.finished = true;
            DestroyWindow(hwnd);
        },
        ID_SAVE => {
            let setting = if state.direct_checked {
                Ok(ProxySetting::Direct)
            } else {
                ProxySetting::proxy(unsafe { window_text(state.edit) })
            };
            match setting {
                Ok(setting) => unsafe {
                    state.result = Some(setting);
                    state.finished = true;
                    DestroyWindow(hwnd);
                },
                Err(error) => unsafe {
                    MessageBoxW(
                        hwnd,
                        wide(&error).as_ptr(),
                        wide("代理地址无效").as_ptr(),
                        0x10,
                    );
                    SetFocus(state.edit);
                },
            }
        }
        _ => {}
    }
}

unsafe fn draw_direct_checkbox(parent: HWND, l_param: LPARAM) -> LRESULT {
    let Some(draw) = (unsafe { (l_param as *mut DRAWITEMSTRUCT).as_ref() }) else {
        return 0;
    };
    let Some(state) = (unsafe { state(parent) }) else {
        return 0;
    };
    if draw.hwndItem != state.direct {
        return 0;
    }

    unsafe {
        FillRect(draw.hDC, &draw.rcItem, GetSysColorBrush(COLOR_BTNFACE));

        let box_size = scale(18, state.dpi);
        let box_top = draw.rcItem.top + (draw.rcItem.bottom - draw.rcItem.top - box_size) / 2;
        let mut checkbox = RECT {
            left: draw.rcItem.left,
            top: box_top,
            right: draw.rcItem.left + box_size,
            bottom: box_top + box_size,
        };
        let mut checkbox_state = DFCS_BUTTONCHECK;
        if state.direct_checked {
            checkbox_state |= DFCS_CHECKED;
        }
        if draw.itemState & ODS_SELECTED != 0 {
            checkbox_state |= DFCS_PUSHED;
        }
        DrawFrameControl(draw.hDC, &mut checkbox, DFC_BUTTON, checkbox_state);

        let font = SendMessageW(draw.hwndItem, WM_GETFONT, 0, 0) as HFONT;
        let old_font = SelectObject(draw.hDC, font);
        SetBkMode(draw.hDC, TRANSPARENT as i32);
        SetTextColor(draw.hDC, GetSysColor(COLOR_BTNTEXT));
        let label = wide("不使用代理（直接连接）");
        let mut text_rect = RECT {
            left: checkbox.right + scale(8, state.dpi),
            top: draw.rcItem.top,
            right: draw.rcItem.right,
            bottom: draw.rcItem.bottom,
        };
        DrawTextW(
            draw.hDC,
            label.as_ptr(),
            (label.len() - 1) as i32,
            &mut text_rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(draw.hDC, old_font);

        if draw.itemState & ODS_FOCUS != 0 {
            let focus = RECT {
                left: text_rect.left - scale(3, state.dpi),
                top: text_rect.top + scale(2, state.dpi),
                right: text_rect.right,
                bottom: text_rect.bottom - scale(2, state.dpi),
            };
            DrawFocusRect(draw.hDC, &focus);
        }
    }
    1
}

unsafe fn state(hwnd: HWND) -> Option<&'static mut State> {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut State;
    unsafe { pointer.as_mut() }
}

unsafe fn window_text(hwnd: HWND) -> String {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..copied as usize])
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn scale(value: i32, dpi: u32) -> i32 {
    ((value as i64 * dpi as i64 + 48) / 96) as i32
}
