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
const WINDOW_HEIGHT: i32 = 320;
const WM_DESTROY: Uint = 0x0002;
const WM_CLOSE: Uint = 0x0010;
const WM_SETFONT: Uint = 0x0030;
const WM_COMMAND: Uint = 0x0111;
const BM_GETCHECK: Uint = 0x00f0;
const BM_SETCHECK: Uint = 0x00f1;
const BST_CHECKED: Wparam = 1;
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
const BS_AUTOCHECKBOX: Dword = 3;
const SW_SHOW: i32 = 5;
const GWLP_USERDATA: i32 = -21;
const COLOR_BTNFACE: usize = 15;
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

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
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
    fn GetSystemMetrics(index: i32) -> i32;
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
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Hinstance;
}

struct State {
    edit: Hwnd,
    direct: Hwnd,
    result: Option<ProxySetting>,
    finished: bool,
}

pub fn shift_pressed() -> bool {
    unsafe { GetAsyncKeyState(VK_SHIFT) < 0 }
}

pub fn show(current: &ProxySetting) -> Result<Option<ProxySetting>, String> {
    unsafe {
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
                32,
                32,
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
            result: None,
            finished: false,
        };
        let x = (GetSystemMetrics(0) - WINDOW_WIDTH) / 2;
        let y = (GetSystemMetrics(1) - WINDOW_HEIGHT) / 2;
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            class_name.as_ptr(),
            wide("startChatGPT 代理设置").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            x,
            y,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
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
            -16,
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
            30,
            28,
            500,
            24,
            0,
            0,
            font,
        );
        state.edit = create_control(
            hwnd,
            instance,
            "EDIT",
            current.proxy_url().unwrap_or(DEFAULT_PROXY),
            30,
            58,
            510,
            36,
            WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL,
            0,
            font,
        );
        create_control(
            hwnd,
            instance,
            "STATIC",
            "支持 http、https、socks4 和 socks5，例如：http://127.0.0.1:10808",
            30,
            102,
            510,
            24,
            0,
            0,
            font,
        );
        state.direct = create_control(
            hwnd,
            instance,
            "BUTTON",
            "不使用代理（直接连接）",
            30,
            138,
            250,
            30,
            WS_TABSTOP | BS_AUTOCHECKBOX,
            ID_DIRECT,
            font,
        );
        create_control(
            hwnd,
            instance,
            "BUTTON",
            "恢复默认",
            30,
            210,
            110,
            38,
            WS_TABSTOP | BS_PUSHBUTTON,
            ID_DEFAULT,
            font,
        );
        create_control(
            hwnd,
            instance,
            "BUTTON",
            "取消",
            330,
            210,
            90,
            38,
            WS_TABSTOP | BS_PUSHBUTTON,
            ID_CANCEL,
            font,
        );
        create_control(
            hwnd,
            instance,
            "BUTTON",
            "保存并启动",
            430,
            210,
            110,
            38,
            WS_TABSTOP | BS_DEFPUSHBUTTON,
            ID_SAVE,
            font,
        );

        if matches!(current, ProxySetting::Direct) {
            SendMessageW(state.direct, BM_SETCHECK, BST_CHECKED, 0);
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
            let direct =
                unsafe { SendMessageW(state.direct, BM_GETCHECK, 0, 0) } as Wparam == BST_CHECKED;
            unsafe { EnableWindow(state.edit, (!direct).into()) };
            if !direct {
                unsafe { SetFocus(state.edit) };
            }
        }
        ID_DEFAULT => unsafe {
            SetWindowTextW(state.edit, wide(DEFAULT_PROXY).as_ptr());
            SendMessageW(state.direct, BM_SETCHECK, 0, 0);
            EnableWindow(state.edit, 1);
            SetFocus(state.edit);
        },
        ID_CANCEL => unsafe {
            state.finished = true;
            DestroyWindow(hwnd);
        },
        ID_SAVE => {
            let direct =
                unsafe { SendMessageW(state.direct, BM_GETCHECK, 0, 0) } as Wparam == BST_CHECKED;
            let setting = if direct {
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
