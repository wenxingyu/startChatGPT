use std::ffi::{OsStr, c_void};
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicUsize, Ordering};

type Bool = i32;
type Dword = u32;
type Handle = *mut c_void;
type Hbitmap = Handle;
type Hbrush = Handle;
type Hcursor = Handle;
type Hdc = Handle;
type Hgdiobj = Handle;
type Hicon = Handle;
type Hinstance = Handle;
type Hmenu = Handle;
type Hrgn = Handle;
type Hwnd = Handle;
type Lparam = isize;
type Lresult = isize;
type Uint = u32;
type Wparam = usize;

const WIDTH: i32 = 460;
const HEIGHT: i32 = 250;
const WM_DESTROY: Uint = 0x0002;
const WM_PAINT: Uint = 0x000f;
const WM_ERASEBKGND: Uint = 0x0014;
const WS_POPUP: Dword = 0x8000_0000;
const WS_EX_TOPMOST: Dword = 0x0000_0008;
const WS_EX_TOOLWINDOW: Dword = 0x0000_0080;
const WS_EX_NOACTIVATE: Dword = 0x0800_0000;
const CS_DROPSHADOW: Uint = 0x0002_0000;
const SW_SHOWNOACTIVATE: i32 = 4;
const PM_REMOVE: Uint = 0x0001;
const IMAGE_ICON: Uint = 1;
const LR_DEFAULTCOLOR: Uint = 0;
const LR_SHARED: Uint = 0x0000_8000;
const DI_NORMAL: Uint = 0x0003;
const TRANSPARENT: i32 = 1;
const FW_SEMIBOLD: i32 = 600;
const DEFAULT_CHARSET: Dword = 1;
const CLEARTYPE_QUALITY: Dword = 5;
const SRCCOPY: Dword = 0x00cc_0020;
const GRADIENT_FILL_RECT_H: Dword = 0;
const GRADIENT_FILL_RECT_V: Dword = 1;
const GW_OWNER: Uint = 4;
const PROCESS_QUERY_LIMITED_INFORMATION: Dword = 0x1000;
const DWMWA_USE_IMMERSIVE_DARK_MODE: Dword = 20;
const DWMWA_WINDOW_CORNER_PREFERENCE: Dword = 33;
const DWMWA_BORDER_COLOR: Dword = 34;
const DWMWCP_ROUND: Dword = 2;
const DWMWA_COLOR_NONE: Dword = 0xffff_fffe;

static PHASE: AtomicUsize = AtomicUsize::new(0);

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Size {
    cx: i32,
    cy: i32,
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
struct PaintStruct {
    hdc: Hdc,
    erase: Bool,
    paint: Rect,
    restore: Bool,
    inc_update: Bool,
    reserved: [u8; 32],
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
    icon: Hicon,
    cursor: Hcursor,
    background: Hbrush,
    menu_name: *const u16,
    class_name: *const u16,
}

#[repr(C)]
struct TriVertex {
    x: i32,
    y: i32,
    red: u16,
    green: u16,
    blue: u16,
    alpha: u16,
}

#[repr(C)]
struct GradientRect {
    upper_left: Dword,
    lower_right: Dword,
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
    fn PeekMessageW(message: *mut Message, hwnd: Hwnd, min: Uint, max: Uint, remove: Uint) -> Bool;
    fn TranslateMessage(message: *const Message) -> Bool;
    fn DispatchMessageW(message: *const Message) -> Lresult;
    fn InvalidateRect(hwnd: Hwnd, rect: *const Rect, erase: Bool) -> Bool;
    fn BeginPaint(hwnd: Hwnd, paint: *mut PaintStruct) -> Hdc;
    fn EndPaint(hwnd: Hwnd, paint: *const PaintStruct) -> Bool;
    fn FillRect(hdc: Hdc, rect: *const Rect, brush: Hbrush) -> i32;
    fn DrawIconEx(
        hdc: Hdc,
        x: i32,
        y: i32,
        icon: Hicon,
        width: i32,
        height: i32,
        step: Uint,
        brush: Hbrush,
        flags: Uint,
    ) -> Bool;
    fn LoadImageW(
        instance: Hinstance,
        name: *const u16,
        kind: Uint,
        width: i32,
        height: i32,
        flags: Uint,
    ) -> Handle;
    fn LoadCursorW(instance: Hinstance, name: *const u16) -> Hcursor;
    fn GetSystemMetrics(index: i32) -> i32;
    fn GetDpiForSystem() -> Uint;
    fn GetDpiForWindow(hwnd: Hwnd) -> Uint;
    fn SetThreadDpiAwarenessContext(context: Handle) -> Handle;
    fn SetWindowRgn(hwnd: Hwnd, region: Hrgn, redraw: Bool) -> i32;
    fn EnumWindows(callback: unsafe extern "system" fn(Hwnd, Lparam) -> Bool, data: Lparam)
    -> Bool;
    fn IsWindowVisible(hwnd: Hwnd) -> Bool;
    fn GetWindow(hwnd: Hwnd, command: Uint) -> Hwnd;
    fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> Bool;
    fn GetWindowThreadProcessId(hwnd: Hwnd, process_id: *mut Dword) -> Dword;
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: Hwnd,
        attribute: Dword,
        value: *const c_void,
        value_size: Dword,
    ) -> i32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateRoundRectRgn(
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        width: i32,
        height: i32,
    ) -> Hrgn;
    fn CreateCompatibleDC(hdc: Hdc) -> Hdc;
    fn CreateCompatibleBitmap(hdc: Hdc, width: i32, height: i32) -> Hbitmap;
    fn SelectObject(hdc: Hdc, object: Hgdiobj) -> Hgdiobj;
    fn DeleteObject(object: Hgdiobj) -> Bool;
    fn DeleteDC(hdc: Hdc) -> Bool;
    fn CreateSolidBrush(color: Dword) -> Hbrush;
    fn SetBkMode(hdc: Hdc, mode: i32) -> i32;
    fn SetTextColor(hdc: Hdc, color: Dword) -> Dword;
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
    ) -> Handle;
    fn GetTextExtentPoint32W(hdc: Hdc, text: *const u16, length: i32, size: *mut Size) -> Bool;
    fn TextOutW(hdc: Hdc, x: i32, y: i32, text: *const u16, length: i32) -> Bool;
    fn Ellipse(hdc: Hdc, left: i32, top: i32, right: i32, bottom: i32) -> Bool;
    fn BitBlt(
        destination: Hdc,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        source: Hdc,
        source_x: i32,
        source_y: i32,
        operation: Dword,
    ) -> Bool;
}

#[link(name = "msimg32")]
unsafe extern "system" {
    fn GradientFill(
        hdc: Hdc,
        vertices: *const TriVertex,
        vertex_count: Dword,
        mesh: *const c_void,
        mesh_count: Dword,
        mode: Dword,
    ) -> Bool;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Hinstance;
    fn OpenProcess(access: Dword, inherit: Bool, process_id: Dword) -> Handle;
    fn QueryFullProcessImageNameW(
        process: Handle,
        flags: Dword,
        path: *mut u16,
        size: *mut Dword,
    ) -> Bool;
    fn CloseHandle(handle: Handle) -> Bool;
}

pub struct Splash {
    hwnd: Hwnd,
    previous_dpi_context: Handle,
}

unsafe fn enable_dwm_rounding(hwnd: Hwnd) -> bool {
    let corner = DWMWCP_ROUND;
    let dark_mode: Dword = 1;
    let border = DWMWA_COLOR_NONE;
    let corner_result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&corner as *const Dword).cast(),
            size_of::<Dword>() as Dword,
        )
    };
    if corner_result < 0 {
        return false;
    }

    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark_mode as *const Dword).cast(),
            size_of::<Dword>() as Dword,
        );
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            (&border as *const Dword).cast(),
            size_of::<Dword>() as Dword,
        );
    }
    true
}

impl Splash {
    pub fn new() -> Option<Self> {
        unsafe {
            // Keep the settings window unchanged, but render the splash itself at the
            // monitor's native DPI instead of letting Windows bitmap-scale it.
            let previous_dpi_context =
                SetThreadDpiAwarenessContext(std::ptr::without_provenance_mut((-4isize) as usize));
            let dpi = GetDpiForSystem().max(96);
            let width = scale(WIDTH, dpi);
            let height = scale(HEIGHT, dpi);
            let instance = GetModuleHandleW(null());
            let class_name = wide("StartChatGPTSplash");
            let class = WindowClass {
                style: CS_DROPSHADOW,
                window_proc: Some(window_proc),
                class_extra: 0,
                window_extra: 0,
                instance,
                icon: null_mut(),
                cursor: LoadCursorW(null_mut(), std::ptr::without_provenance(32512)),
                background: null_mut(),
                menu_name: null(),
                class_name: class_name.as_ptr(),
            };
            RegisterClassW(&class);

            let x = (GetSystemMetrics(0) - width) / 2;
            let y = (GetSystemMetrics(1) - height) / 2;
            let mut ex_style = WS_EX_TOPMOST;
            if option_env!("STARTCHATGPT_SPLASH_PREVIEW").is_none() {
                ex_style |= WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
            }
            let hwnd = CreateWindowExW(
                ex_style,
                class_name.as_ptr(),
                wide("正在启动 ChatGPT").as_ptr(),
                WS_POPUP,
                x,
                y,
                width,
                height,
                null_mut(),
                null_mut(),
                instance,
                null_mut(),
            );
            if hwnd.is_null() {
                if !previous_dpi_context.is_null() {
                    SetThreadDpiAwarenessContext(previous_dpi_context);
                }
                return None;
            }

            if !enable_dwm_rounding(hwnd) {
                let radius = scale(40, dpi);
                let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, radius, radius);
                SetWindowRgn(hwnd, region, 1);
            }
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            UpdateWindow(hwnd);
            Some(Self {
                hwnd,
                previous_dpi_context,
            })
        }
    }

    pub fn pump(&mut self) {
        PHASE.fetch_add(1, Ordering::Relaxed);
        unsafe {
            let mut message: Message = zeroed();
            while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            InvalidateRect(self.hwnd, null(), 0);
        }
    }
}

impl Drop for Splash {
    fn drop(&mut self) {
        unsafe {
            if !self.hwnd.is_null() {
                DestroyWindow(self.hwnd);
            }
            if !self.previous_dpi_context.is_null() {
                SetThreadDpiAwarenessContext(self.previous_dpi_context);
            }
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: Hwnd,
    message: Uint,
    w_param: Wparam,
    l_param: Lparam,
) -> Lresult {
    match message {
        WM_PAINT => {
            unsafe { paint(hwnd) };
            0
        }
        WM_ERASEBKGND => 1,
        WM_DESTROY => 0,
        _ => unsafe { DefWindowProcW(hwnd, message, w_param, l_param) },
    }
}

unsafe fn paint(hwnd: Hwnd) {
    unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96);
        let width = scale(WIDTH, dpi);
        let height = scale(HEIGHT, dpi);
        let mut paint: PaintStruct = zeroed();
        let target = BeginPaint(hwnd, &mut paint);
        let buffer = CreateCompatibleDC(target);
        let bitmap = CreateCompatibleBitmap(target, width, height);
        let old_bitmap = SelectObject(buffer, bitmap);

        fill_gradient(
            buffer,
            width,
            height,
            (43, 53, 66),
            (19, 23, 31),
            GRADIENT_FILL_RECT_V,
        );

        let accent_vertices = [
            gradient_vertex(0, 0, (16, 163, 127)),
            gradient_vertex(width, scale(3, dpi), (112, 87, 255)),
        ];
        let accent_mesh = GradientRect {
            upper_left: 0,
            lower_right: 1,
        };
        GradientFill(
            buffer,
            accent_vertices.as_ptr(),
            accent_vertices.len() as Dword,
            (&accent_mesh as *const GradientRect).cast(),
            1,
            GRADIENT_FILL_RECT_H,
        );

        let instance = GetModuleHandleW(null());
        let icon_size = scale(68, dpi);
        let icon = LoadImageW(
            instance,
            std::ptr::without_provenance(1),
            IMAGE_ICON,
            icon_size,
            icon_size,
            LR_DEFAULTCOLOR | LR_SHARED,
        );
        if !icon.is_null() {
            DrawIconEx(
                buffer,
                (width - icon_size) / 2,
                scale(24, dpi),
                icon,
                icon_size,
                icon_size,
                0,
                null_mut(),
                DI_NORMAL,
            );
        }

        SetBkMode(buffer, TRANSPARENT);
        draw_centered_text(
            buffer,
            "正在启动 ChatGPT",
            scale(108, dpi),
            scale(22, dpi),
            FW_SEMIBOLD,
            rgb(247, 249, 250),
            width,
        );
        draw_spinner(buffer, dpi, width);
        draw_centered_text(
            buffer,
            "正在连接本地代理  127.0.0.1:10808",
            scale(215, dpi),
            scale(14, dpi),
            400,
            rgb(174, 184, 193),
            width,
        );

        BitBlt(target, 0, 0, width, height, buffer, 0, 0, SRCCOPY);
        SelectObject(buffer, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(buffer);
        EndPaint(hwnd, &paint);
    }
}

unsafe fn draw_centered_text(
    hdc: Hdc,
    text: &str,
    y: i32,
    size: i32,
    weight: i32,
    color: Dword,
    width: i32,
) {
    unsafe {
        let face = wide("Segoe UI");
        let font = CreateFontW(
            -size,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            0,
            0,
            CLEARTYPE_QUALITY,
            0,
            face.as_ptr(),
        );
        let old_font = SelectObject(hdc, font);
        SetTextColor(hdc, color);
        let encoded: Vec<u16> = OsStr::new(text).encode_wide().collect();
        let mut extent = Size { cx: 0, cy: 0 };
        GetTextExtentPoint32W(hdc, encoded.as_ptr(), encoded.len() as i32, &mut extent);
        TextOutW(
            hdc,
            (width - extent.cx) / 2,
            y,
            encoded.as_ptr(),
            encoded.len() as i32,
        );
        SelectObject(hdc, old_font);
        DeleteObject(font);
    }
}

unsafe fn draw_spinner(hdc: Hdc, dpi: Uint, width: i32) {
    const DOTS: usize = 10;
    let phase = PHASE.load(Ordering::Relaxed) % DOTS;
    for index in 0..DOTS {
        let angle = index as f64 * std::f64::consts::TAU / DOTS as f64;
        let radius = scale(21, dpi) as f64;
        let dot_radius = scale(3, dpi);
        let x = width / 2 + (angle.cos() * radius) as i32;
        let y = scale(174, dpi) + (angle.sin() * radius) as i32;
        let distance = (index + DOTS - phase) % DOTS;
        let intensity = 230u8.saturating_sub((distance as u8) * 18).max(65);
        let brush = unsafe {
            CreateSolidBrush(rgb(
                (intensity as u16 * 45 / 100) as u8,
                intensity,
                (intensity as u16 * 82 / 100) as u8,
            ))
        };
        let old_brush = unsafe { SelectObject(hdc, brush) };
        unsafe {
            Ellipse(
                hdc,
                x - dot_radius,
                y - dot_radius,
                x + dot_radius + 1,
                y + dot_radius + 1,
            )
        };
        unsafe {
            SelectObject(hdc, old_brush);
            DeleteObject(brush);
        }
    }
}

unsafe fn fill_gradient(
    hdc: Hdc,
    width: i32,
    height: i32,
    top: (u8, u8, u8),
    bottom: (u8, u8, u8),
    mode: Dword,
) {
    let vertices = [
        gradient_vertex(0, 0, top),
        gradient_vertex(width, height, bottom),
    ];
    let mesh = GradientRect {
        upper_left: 0,
        lower_right: 1,
    };
    let filled = unsafe {
        GradientFill(
            hdc,
            vertices.as_ptr(),
            vertices.len() as Dword,
            (&mesh as *const GradientRect).cast(),
            1,
            mode,
        )
    };
    if filled == 0 {
        let brush = unsafe { CreateSolidBrush(rgb(top.0, top.1, top.2)) };
        unsafe {
            FillRect(
                hdc,
                &Rect {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                },
                brush,
            );
            DeleteObject(brush);
        }
    }
}

const fn gradient_vertex(x: i32, y: i32, color: (u8, u8, u8)) -> TriVertex {
    TriVertex {
        x,
        y,
        red: (color.0 as u16) << 8,
        green: (color.1 as u16) << 8,
        blue: (color.2 as u16) << 8,
        alpha: 0,
    }
}

pub fn has_visible_window_for(executable: &Path) -> bool {
    let target = executable.to_string_lossy().replace('/', "\\");
    let mut search = WindowSearch {
        target: target.to_lowercase(),
        found: false,
    };
    unsafe {
        EnumWindows(enum_window, (&mut search as *mut WindowSearch) as Lparam);
    }
    search.found
}

struct WindowSearch {
    target: String,
    found: bool,
}

unsafe extern "system" fn enum_window(hwnd: Hwnd, data: Lparam) -> Bool {
    unsafe {
        let search = &mut *(data as *mut WindowSearch);
        if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
            return 1;
        }

        let mut rect = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetWindowRect(hwnd, &mut rect) == 0
            || rect.right - rect.left < 240
            || rect.bottom - rect.top < 160
        {
            return 1;
        }

        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, &mut process_id);
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id);
        if process.is_null() {
            return 1;
        }

        let mut path = vec![0u16; 32_768];
        let mut length = path.len() as Dword;
        let success = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length);
        CloseHandle(process);
        if success != 0 {
            let actual = String::from_utf16_lossy(&path[..length as usize]).to_lowercase();
            if actual == search.target {
                search.found = true;
                return 0;
            }
        }
        1
    }
}

const fn rgb(red: u8, green: u8, blue: u8) -> Dword {
    red as Dword | ((green as Dword) << 8) | ((blue as Dword) << 16)
}

fn scale(value: i32, dpi: Uint) -> i32 {
    ((value as i64 * dpi as i64 + 48) / 96) as i32
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}
