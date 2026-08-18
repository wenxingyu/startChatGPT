use crate::config::{DEFAULT_PROXY, ProxySetting};
use std::ffi::{OsStr, c_void};
use std::mem::zeroed;
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};

type Bool = i32;
type Dword = u32;
type Handle = *mut c_void;
type Hbrush = Handle;
type Hcursor = Handle;
type Hfont = Handle;
type Hinstance = Handle;
type Hmenu = Handle;
type Hwnd = Handle;
type Lparam = isize;
type Lresult = isize;
type Uint = u32;
type Wparam = usize;

const WINDOW_WIDTH: i32 = 580;
const WINDOW_HEIGHT: i32 = 290;
const WM_DESTROY: Uint = 0x0002;
const WM_CLOSE: Uint = 0x0010;
const WM_DRAWITEM: Uint = 0x002b;
const WM_SETFONT: Uint = 0x0030;
const WM_GETFONT: Uint = 0x0031;
const WM_COMMAND: Uint = 0x0111;
const WS_OVERLAPPED: Dword = 0;
const WS_CAPTION: Dword = 0x00c0_0000;
const WS_SYSMENU: Dword = 0x0008_0000;
const WS_CHILD: Dword = 0x4000_0000;
const WS_VISIBLE: Dword = 0x1000_0000;
const WS_BORDER: Dword = 0x0080_0000;
const WS_TABSTOP: Dword = 0x0001_0000;
const WS_EX_APPWINDOW: Dword = 0x0004_0000;
const ES_AUTOHSCROLL: Dword = 0x0080;
const BS_PUSHBUTTON: Dword = 0;
const BS_DEFPUSHBUTTON: Dword = 1;
const BS_OWNERDRAW: Dword = 11;
const SW_SHOW: i32 = 5;
const GWLP_USERDATA: i32 = -21;
const COLOR_BTNFACE: usize = 15;
const COLOR_BTNTEXT: i32 = 18;
const IMAGE_ICON: Uint = 1;
const LR_SHARED: Uint = 0x0000_8000;
const DEFAULT_CHARSET: Dword = 1;
const CLEARTYPE_QUALITY: Dword = 5;
const FW_NORMAL: i32 = 400;
const ID_SAVE: u16 = 1001;
const ID_CANCEL: u16 = 1002;
const ID_DIRECT: u16 = 1003;
const ID_DEFAULT: u16 = 1004;
const VK_SHIFT: i32 = 0x10;
const ODS_SELECTED: Uint = 0x0001;
const ODS_FOCUS: Uint = 0x0010;
const DFC_BUTTON: Uint = 4;
const DFCS_BUTTONCHECK: Uint = 0;
const DFCS_PUSHED: Uint = 0x0200;
const DFCS_CHECKED: Uint = 0x0400;
const DT_LEFT: Uint = 0;
const DT_VCENTER: Uint = 0x0004;
const DT_SINGLELINE: Uint = 0x0020;
const DT_NOPREFIX: Uint = 0x0800;
const TRANSPARENT: i32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct DrawItemStruct {
    control_type: Uint,
    control_id: Uint,
    item_id: Uint,
    item_action: Uint,
    item_state: Uint,
    hwnd_item: Hwnd,
    hdc: Handle,
    rect: Rect,
    item_data: usize,
}

#[repr(C)]
struct Message {
    hwnd: Hwnd,
    message: Uint,
    w_param: Wparam,
    l_param: Lparam,
    time: Dword,
    point: Point,
    private: Dword,
}

type WindowProc = unsafe extern "system" fn(Hwnd, Uint, Wparam, Lparam) -> Lresult;

#[repr(C)]
struct WindowClass {
    style: Uint,
    window_proc: Option<WindowProc>,
    class_extra: i32,
    window_extra: i32,
    instance: Hinstance,
    icon: Handle,
    cursor: Hcursor,
    background: Hbrush,
    menu_name: *const u16,
    class_name: *const u16,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassW(class: *const WindowClass) -> u16;
    fn CreateWindowExW(
        ex_style: Dword,
        class_name: *const u16,
        window_name: *const u16,
        style: Dword,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Hwnd,
        menu: Hmenu,
        instance: Hinstance,
        parameter: *mut c_void,
    ) -> Hwnd;
    fn DefWindowProcW(hwnd: Hwnd, message: Uint, w_param: Wparam, l_param: Lparam) -> Lresult;
    fn DestroyWindow(hwnd: Hwnd) -> Bool;
    fn ShowWindow(hwnd: Hwnd, command: i32) -> Bool;
    fn UpdateWindow(hwnd: Hwnd) -> Bool;
    fn GetMessageW(message: *mut Message, hwnd: Hwnd, min: Uint, max: Uint) -> Bool;
    fn TranslateMessage(message: *const Message) -> Bool;
    fn DispatchMessageW(message: *const Message) -> Lresult;
    fn PostQuitMessage(code: i32);
    fn SendMessageW(hwnd: Hwnd, message: Uint, w_param: Wparam, l_param: Lparam) -> Lresult;
    fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
    fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
    fn SetWindowTextW(hwnd: Hwnd, text: *const u16) -> Bool;
    fn GetWindowTextLengthW(hwnd: Hwnd) -> i32;
    fn GetWindowTextW(hwnd: Hwnd, text: *mut u16, maximum: i32) -> i32;
    fn EnableWindow(hwnd: Hwnd, enable: Bool) -> Bool;
    fn SetFocus(hwnd: Hwnd) -> Hwnd;
    fn InvalidateRect(hwnd: Hwnd, rect: *const Rect, erase: Bool) -> Bool;
    fn GetSystemMetrics(index: i32) -> i32;
    fn GetDpiForSystem() -> Uint;
    fn SetProcessDPIAware() -> Bool;
    fn LoadCursorW(instance: Hinstance, name: *const u16) -> Hcursor;
    fn LoadImageW(
        instance: Hinstance,
        name: *const u16,
        kind: Uint,
        width: i32,
        height: i32,
        flags: Uint,
    ) -> Handle;
    fn MessageBoxW(hwnd: Hwnd, text: *const u16, caption: *const u16, kind: Uint) -> i32;
    fn GetAsyncKeyState(key: i32) -> i16;
    fn DrawFrameControl(hdc: Handle, rect: *mut Rect, kind: Uint, state: Uint) -> Bool;
    fn DrawFocusRect(hdc: Handle, rect: *const Rect) -> Bool;
    fn DrawTextW(hdc: Handle, text: *const u16, length: i32, rect: *mut Rect, format: Uint) -> i32;
    fn FillRect(hdc: Handle, rect: *const Rect, brush: Hbrush) -> i32;
    fn GetSysColorBrush(index: i32) -> Hbrush;
    fn GetSysColor(index: i32) -> Dword;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateFontW(
        height: i32,
        width: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: Dword,
        underline: Dword,
        strikeout: Dword,
        charset: Dword,
        output_precision: Dword,
        clip_precision: Dword,
        quality: Dword,
        pitch_and_family: Dword,
        face: *const u16,
    ) -> Hfont;
    fn DeleteObject(object: Handle) -> Bool;
    fn SelectObject(hdc: Handle, object: Handle) -> Handle;
    fn SetBkMode(hdc: Handle, mode: i32) -> i32;
    fn SetTextColor(hdc: Handle, color: Dword) -> Dword;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Hinstance;
}

struct State {
    edit: Hwnd,
    direct: Hwnd,
    direct_checked: bool,
    dpi: Uint,
    result: Option<ProxySetting>,
    finished: bool,
}

pub fn shift_pressed() -> bool {
    unsafe { GetAsyncKeyState(VK_SHIFT) < 0 }
}

pub fn show(current: &ProxySetting) -> Result<Option<ProxySetting>, String> {
    unsafe {
        SetProcessDPIAware();
        let dpi = GetDpiForSystem().max(96);
        let window_width = scale(WINDOW_WIDTH, dpi);
        let window_height = scale(WINDOW_HEIGHT, dpi);
        let instance = GetModuleHandleW(null());
        let class_name = wide("StartChatGPTSettings");
        let class = WindowClass {
            style: 0,
            window_proc: Some(window_proc),
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: LoadImageW(
                instance,
                std::ptr::without_provenance(1),
                IMAGE_ICON,
                scale(32, dpi),
                scale(32, dpi),
                LR_SHARED,
            ),
            cursor: LoadCursorW(null_mut(), std::ptr::without_provenance(32512)),
            background: std::ptr::without_provenance_mut(COLOR_BTNFACE + 1),
            menu_name: null(),
            class_name: class_name.as_ptr(),
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
            FW_NORMAL,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            0,
            0,
            CLEARTYPE_QUALITY,
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
            WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL,
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
            WS_TABSTOP | BS_OWNERDRAW,
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
            WS_TABSTOP | BS_PUSHBUTTON,
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
            WS_TABSTOP | BS_PUSHBUTTON,
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
            WS_TABSTOP | BS_DEFPUSHBUTTON,
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

        let mut message: Message = zeroed();
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
    parent: Hwnd,
    instance: Hinstance,
    class: &str,
    text: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    style: Dword,
    id: u16,
    font: Hfont,
) -> Hwnd {
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
    unsafe { SendMessageW(hwnd, WM_SETFONT, font as Wparam, 1) };
    hwnd
}

unsafe extern "system" fn window_proc(
    hwnd: Hwnd,
    message: Uint,
    w_param: Wparam,
    l_param: Lparam,
) -> Lresult {
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

unsafe fn handle_command(hwnd: Hwnd, id: u16) {
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

unsafe fn draw_direct_checkbox(parent: Hwnd, l_param: Lparam) -> Lresult {
    let Some(draw) = (unsafe { (l_param as *mut DrawItemStruct).as_ref() }) else {
        return 0;
    };
    let Some(state) = (unsafe { state(parent) }) else {
        return 0;
    };
    if draw.hwnd_item != state.direct {
        return 0;
    }

    unsafe {
        FillRect(draw.hdc, &draw.rect, GetSysColorBrush(COLOR_BTNFACE as i32));

        let box_size = scale(18, state.dpi);
        let box_top = draw.rect.top + (draw.rect.bottom - draw.rect.top - box_size) / 2;
        let mut checkbox = Rect {
            left: draw.rect.left,
            top: box_top,
            right: draw.rect.left + box_size,
            bottom: box_top + box_size,
        };
        let mut checkbox_state = DFCS_BUTTONCHECK;
        if state.direct_checked {
            checkbox_state |= DFCS_CHECKED;
        }
        if draw.item_state & ODS_SELECTED != 0 {
            checkbox_state |= DFCS_PUSHED;
        }
        DrawFrameControl(draw.hdc, &mut checkbox, DFC_BUTTON, checkbox_state);

        let font = SendMessageW(draw.hwnd_item, WM_GETFONT, 0, 0) as Handle;
        let old_font = SelectObject(draw.hdc, font);
        SetBkMode(draw.hdc, TRANSPARENT);
        SetTextColor(draw.hdc, GetSysColor(COLOR_BTNTEXT));
        let label = wide("不使用代理（直接连接）");
        let mut text_rect = Rect {
            left: checkbox.right + scale(8, state.dpi),
            top: draw.rect.top,
            right: draw.rect.right,
            bottom: draw.rect.bottom,
        };
        DrawTextW(
            draw.hdc,
            label.as_ptr(),
            (label.len() - 1) as i32,
            &mut text_rect,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(draw.hdc, old_font);

        if draw.item_state & ODS_FOCUS != 0 {
            let focus = Rect {
                left: text_rect.left - scale(3, state.dpi),
                top: text_rect.top + scale(2, state.dpi),
                right: text_rect.right,
                bottom: text_rect.bottom - scale(2, state.dpi),
            };
            DrawFocusRect(draw.hdc, &focus);
        }
    }
    1
}

unsafe fn state(hwnd: Hwnd) -> Option<&'static mut State> {
    let pointer = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut State;
    unsafe { pointer.as_mut() }
}

unsafe fn window_text(hwnd: Hwnd) -> String {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..copied as usize])
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn scale(value: i32, dpi: Uint) -> i32 {
    ((value as i64 * dpi as i64 + 48) / 96) as i32
}
