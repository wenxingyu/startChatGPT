use std::cell::RefCell;
use std::mem::{size_of, zeroed};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicUsize, Ordering};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*},
    System::{LibraryLoader::*, ProcessStatus::*, Threading::*},
    UI::{HiDpi::*, WindowsAndMessaging::*},
};

const WIDTH: i32 = 460;
const HEIGHT: i32 = 250;

static PHASE: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    // The splash and its paint callbacks run on the same UI thread.
    static CONNECTION_TEXT: RefCell<String> = const { RefCell::new(String::new()) };
}

pub struct Splash {
    hwnd: HWND,
}

unsafe fn enable_dwm_rounding(hwnd: HWND) -> bool {
    let corner = DWMWCP_ROUND;
    let dark_mode: u32 = 1;
    let border = DWMWA_COLOR_NONE;
    let corner_result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&corner as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            size_of::<u32>() as u32,
        )
    };
    if corner_result < 0 {
        return false;
    }

    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
            (&dark_mode as *const u32).cast(),
            size_of::<u32>() as u32,
        );
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR as u32,
            (&border as *const u32).cast(),
            size_of::<u32>() as u32,
        );
    }
    true
}

impl Splash {
    pub fn new(proxy_setting: &crate::config::ProxySetting) -> Option<Self> {
        CONNECTION_TEXT.with(|text| {
            *text.borrow_mut() = match proxy_setting.proxy_url() {
                Some(url) => format!("正在通过代理连接  {url}"),
                None => "正在直接连接".into(),
            };
        });
        unsafe {
            // The splash runs in a separate launcher process from the settings window.
            // System DPI awareness keeps text crisp without constructing a raw DPI
            // awareness pseudo-handle at runtime.
            SetProcessDPIAware();
            let dpi = GetDpiForSystem().max(96);
            let width = scale(WIDTH, dpi);
            let height = scale(HEIGHT, dpi);
            let instance = GetModuleHandleW(null());
            let class_name = wide("StartChatGPTSplash");
            let class = WNDCLASSW {
                style: CS_DROPSHADOW,
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                lpszClassName: class_name.as_ptr(),
                ..zeroed()
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
                return None;
            }

            if !enable_dwm_rounding(hwnd) {
                let radius = scale(40, dpi);
                let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, radius, radius);
                SetWindowRgn(hwnd, region, 1);
            }
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            UpdateWindow(hwnd);
            Some(Self { hwnd })
        }
    }

    pub fn pump(&mut self) {
        PHASE.fetch_add(1, Ordering::Relaxed);
        unsafe {
            let mut message: MSG = zeroed();
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
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
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

unsafe fn paint(hwnd: HWND) {
    unsafe {
        let dpi = GetDpiForSystem().max(96);
        let width = scale(WIDTH, dpi);
        let height = scale(HEIGHT, dpi);
        let mut paint: PAINTSTRUCT = zeroed();
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
        let accent_mesh = GRADIENT_RECT {
            UpperLeft: 0,
            LowerRight: 1,
        };
        GradientFill(
            buffer,
            accent_vertices.as_ptr(),
            accent_vertices.len() as u32,
            (&accent_mesh as *const GRADIENT_RECT).cast(),
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

        SetBkMode(buffer, TRANSPARENT as i32);
        draw_centered_text(
            buffer,
            "正在启动 ChatGPT",
            scale(108, dpi),
            scale(22, dpi),
            FW_SEMIBOLD as i32,
            rgb(247, 249, 250),
            width,
        );
        draw_spinner(buffer, dpi, width);
        CONNECTION_TEXT.with(|text| {
            draw_centered_text(
                buffer,
                &text.borrow(),
                scale(215, dpi),
                scale(14, dpi),
                400,
                rgb(174, 184, 193),
                width,
            );
        });

        BitBlt(target, 0, 0, width, height, buffer, 0, 0, SRCCOPY);
        SelectObject(buffer, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(buffer);
        EndPaint(hwnd, &paint);
    }
}

unsafe fn draw_centered_text(
    hdc: HDC,
    text: &str,
    y: i32,
    size: i32,
    weight: i32,
    color: u32,
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
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            face.as_ptr(),
        );
        let old_font = SelectObject(hdc, font);
        SetTextColor(hdc, color);
        let encoded: Vec<u16> = text.encode_utf16().collect();
        let mut extent = SIZE { cx: 0, cy: 0 };
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

unsafe fn draw_spinner(hdc: HDC, dpi: u32, width: i32) {
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
    hdc: HDC,
    width: i32,
    height: i32,
    top: (u8, u8, u8),
    bottom: (u8, u8, u8),
    mode: GRADIENT_FILL,
) {
    let vertices = [
        gradient_vertex(0, 0, top),
        gradient_vertex(width, height, bottom),
    ];
    let mesh = GRADIENT_RECT {
        UpperLeft: 0,
        LowerRight: 1,
    };
    let filled = unsafe {
        GradientFill(
            hdc,
            vertices.as_ptr(),
            vertices.len() as u32,
            (&mesh as *const GRADIENT_RECT).cast(),
            1,
            mode,
        )
    };
    if filled == 0 {
        let brush = unsafe { CreateSolidBrush(rgb(top.0, top.1, top.2)) };
        unsafe {
            FillRect(
                hdc,
                &RECT {
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

const fn gradient_vertex(x: i32, y: i32, color: (u8, u8, u8)) -> TRIVERTEX {
    TRIVERTEX {
        x,
        y,
        Red: (color.0 as u16) << 8,
        Green: (color.1 as u16) << 8,
        Blue: (color.2 as u16) << 8,
        Alpha: 0,
    }
}

pub fn has_visible_window_for(executable: &Path) -> bool {
    let target = executable.to_string_lossy().replace('/', "\\");
    let mut search = WindowSearch {
        target: target.to_lowercase(),
        found: false,
    };
    unsafe {
        EnumWindows(
            Some(enum_window),
            (&mut search as *mut WindowSearch) as LPARAM,
        );
    }
    search.found
}

/// An owned handle that can sleep until a matching process exits.
pub struct ProcessWait(HANDLE);

// Windows process handles may be waited on from any thread.
unsafe impl Send for ProcessWait {}

impl ProcessWait {
    pub fn wait(self) {
        unsafe {
            WaitForSingleObject(self.0, INFINITE);
        }
    }
}

impl Drop for ProcessWait {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

/// Finds a process with this exact executable path and returns a waitable handle.
pub fn process_wait_for(executable: &Path) -> Option<ProcessWait> {
    let target = executable
        .to_string_lossy()
        .replace('/', "\\")
        .to_lowercase();
    let mut process_ids = vec![0u32; 1024];
    loop {
        let mut bytes_returned = 0;
        let capacity_bytes = (process_ids.len() * size_of::<u32>()) as u32;
        let success = unsafe {
            EnumProcesses(
                process_ids.as_mut_ptr(),
                capacity_bytes,
                &mut bytes_returned,
            )
        };
        if success == 0 {
            return None;
        }
        if bytes_returned < capacity_bytes {
            process_ids.truncate(bytes_returned as usize / size_of::<u32>());
            break;
        }
        process_ids.resize(process_ids.len() * 2, 0);
    }

    let mut path = vec![0u16; 32_768];
    for process_id in process_ids {
        if process_id == 0 {
            continue;
        }
        let process = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                0,
                process_id,
            )
        };
        if process.is_null() {
            continue;
        }
        let mut length = path.len() as u32;
        let success =
            unsafe { QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut length) };
        if success != 0
            && String::from_utf16_lossy(&path[..length as usize]).to_lowercase() == target
        {
            return Some(ProcessWait(process));
        }
        unsafe { CloseHandle(process) };
    }
    None
}

struct WindowSearch {
    target: String,
    found: bool,
}

unsafe extern "system" fn enum_window(hwnd: HWND, data: LPARAM) -> i32 {
    unsafe {
        let search = &mut *(data as *mut WindowSearch);
        if IsWindowVisible(hwnd) == 0 || !GetWindow(hwnd, GW_OWNER).is_null() {
            return 1;
        }

        let mut rect = RECT {
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
        let mut length = path.len() as u32;
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

const fn rgb(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn scale(value: i32, dpi: u32) -> i32 {
    ((value as i64 * dpi as i64 + 48) / 96) as i32
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_current_process_by_exact_executable_path() {
        assert!(process_wait_for(&std::env::current_exe().unwrap()).is_some());
    }
}
