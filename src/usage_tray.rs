//! Native notification-area quota icon, owned and positioned by Windows Explorer.
use crate::{config::ProxySetting, usage};
use std::{
    cell::RefCell,
    mem::{size_of, zeroed},
    path::Path,
    ptr::{null, null_mut},
    sync::{Arc, Mutex, mpsc::Sender},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::*,
    UI::{HiDpi::*, Shell::*, WindowsAndMessaging::*},
};

const CALLBACK: u32 = WM_APP + 20;
struct Ui {
    state: Arc<Mutex<usage::State>>,
    action: Sender<usage::Action>,
    added: bool,
    restart: u32,
    details_open: bool,
    last_render: Option<RenderKey>,
}

#[derive(Clone, Eq, PartialEq)]
struct RenderKey {
    values: [Option<i32>; 2],
    stale: bool,
    size: i32,
}
thread_local! { static UI: RefCell<Option<Ui>> = const { RefCell::new(None) }; }
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub fn run(app: &Path, proxy: ProxySetting) -> Result<(), String> {
    unsafe {
        let class = wide("StartChatGPTQuotaTray");
        if !FindWindowW(class.as_ptr(), null()).is_null() {
            return Ok(());
        }
        let instance = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            ..zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return Err("无法注册托盘窗口".into());
        }
        let state = Arc::new(Mutex::new(usage::State::default()));
        let (action, worker) = usage::start(app.to_owned(), proxy, state.clone());
        UI.with(|ui| {
            *ui.borrow_mut() = Some(Ui {
                state,
                action: action.clone(),
                added: false,
                restart: RegisterWindowMessageW(wide("TaskbarCreated").as_ptr()),
                details_open: false,
                last_render: None,
            })
        });
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.as_ptr(),
            wide("Codex 额度托盘").as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        let mut success = false;
        if !hwnd.is_null() {
            success = update(hwnd);
            if success {
                SetTimer(hwnd, 1, 3000, None);
                let mut msg: MSG = zeroed();
                while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            } else {
                DestroyWindow(hwnd);
            }
        }
        let _ = action.send(usage::Action::Stop);
        let _ = worker.join();
        UI.with(|ui| *ui.borrow_mut() = None);
        if success {
            Ok(())
        } else {
            Err("无法添加 Codex 系统托盘图标".into())
        }
    }
}

fn icon_text(state: &usage::State) -> String {
    state.windows[0]
        .as_ref()
        .map(|w| format!("{:.0}", w.remaining))
        .unwrap_or("--".into())
}

unsafe fn icon_size(hwnd: HWND) -> i32 {
    unsafe {
        // The hidden owner window may be on another monitor. Query the actual
        // notification icon first, then the taskbar while the icon is being added.
        let id = NOTIFYICONIDENTIFIER {
            cbSize: size_of::<NOTIFYICONIDENTIFIER>() as u32,
            hWnd: hwnd,
            uID: 1,
            ..zeroed()
        };
        let mut rect: RECT = zeroed();
        let mut dpi = 0;
        if Shell_NotifyIconGetRect(&id, &mut rect) == 0 {
            let monitor = MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST);
            let mut dpi_y = 0;
            if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y) != 0 {
                dpi = 0;
            }
        }
        if dpi == 0 {
            let taskbar = FindWindowW(wide("Shell_TrayWnd").as_ptr(), null());
            dpi = GetDpiForWindow(taskbar);
        }
        if dpi == 0 {
            dpi = GetDpiForSystem();
        }
        GetSystemMetricsForDpi(SM_CXSMICON, dpi.max(96)).max(16)
    }
}

unsafe fn numeral_bounds(dc: HDC, value: &str) -> Option<RECT> {
    unsafe {
        let one = FIXED { fract: 0, value: 1 };
        let matrix = MAT2 {
            eM11: one,
            eM12: zeroed(),
            eM21: zeroed(),
            eM22: one,
        };
        let mut metrics: TEXTMETRICW = zeroed();
        if GetTextMetricsW(dc, &mut metrics) == 0 {
            return None;
        }
        let mut bounds = RECT {
            left: i32::MAX,
            top: i32::MAX,
            right: i32::MIN,
            bottom: i32::MIN,
        };
        let mut pen = 0;
        for ch in value.encode_utf16() {
            let mut glyph: GLYPHMETRICS = zeroed();
            if GetGlyphOutlineW(
                dc,
                ch as u32,
                GGO_METRICS,
                &mut glyph,
                0,
                null_mut(),
                &matrix,
            ) == GDI_ERROR as u32
            {
                return None;
            }
            let left = pen + glyph.gmptGlyphOrigin.x;
            let top = metrics.tmAscent - glyph.gmptGlyphOrigin.y;
            bounds.left = bounds.left.min(left);
            bounds.top = bounds.top.min(top);
            bounds.right = bounds.right.max(left + glyph.gmBlackBoxX as i32);
            bounds.bottom = bounds.bottom.max(top + glyph.gmBlackBoxY as i32);
            pen += glyph.gmCellIncX as i32;
        }
        Some(bounds)
    }
}

unsafe fn make_icon(state: &usage::State, size: i32) -> HICON {
    unsafe {
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = size;
        info.bmiHeader.biHeight = -size;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() {
            return null_mut();
        }
        let mut bits = null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if bitmap.is_null() {
            DeleteDC(dc);
            return null_mut();
        }
        let old = SelectObject(dc, bitmap);
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, (size * size) as usize);
        pixels.fill(0xff1c1f24);
        let remaining = state.windows[0].as_ref().map(|w| w.remaining);
        let color = if state.stale() {
            0x00afa39a
        } else if remaining.is_some_and(|v| v <= 10.0) {
            0x007e75ff
        } else if remaining.is_some_and(|v| v <= 25.0) {
            0x005ec3f6
        } else {
            0x00aadc66
        };
        let value = icon_text(state);
        // Use a condensed numeral face for three digits, preserving their height
        // instead of shrinking the entire "100" to fit a proportional UI font.
        let encoded = wide(&value);
        let mut font_size = size * 3 / 2;
        let (font, old_font, ink) = loop {
            let font = CreateFontW(
                -font_size,
                if value.len() > 2 {
                    (font_size / 3).max(3)
                } else {
                    0
                },
                0,
                0,
                700,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                0,
                0,
                ANTIALIASED_QUALITY as u32,
                0,
                wide("Bahnschrift").as_ptr(),
            );
            let old_font = SelectObject(dc, font);
            let ink = numeral_bounds(dc, &value);
            if ink
                .as_ref()
                .is_some_and(|r| r.right - r.left <= size - 2 && r.bottom - r.top <= size - 3)
                || font_size <= 5
            {
                break (
                    font,
                    old_font,
                    ink.unwrap_or(RECT {
                        left: 0,
                        top: 0,
                        right: size,
                        bottom: size,
                    }),
                );
            }
            SelectObject(dc, old_font);
            DeleteObject(font);
            font_size -= 1;
        };
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, color);
        // Center visible ink rather than the font's line box and unused descender space.
        TextOutW(
            dc,
            (size - (ink.right - ink.left)) / 2 - ink.left,
            (size - (ink.bottom - ink.top)) / 2 - ink.top,
            encoded.as_ptr(),
            (encoded.len() - 1) as i32,
        );
        GdiFlush();
        for pixel in pixels {
            *pixel |= 0xff000000;
        }
        SelectObject(dc, old_font);
        DeleteObject(font);
        SelectObject(dc, old);
        let mask_bytes = vec![0u8; ((size + 15) / 16 * 2 * size) as usize];
        let mask = CreateBitmap(size, size, 1, 1, mask_bytes.as_ptr().cast());
        let icon = if mask.is_null() {
            null_mut()
        } else {
            CreateIconIndirect(&ICONINFO {
                fIcon: 1,
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask,
                hbmColor: bitmap,
            })
        };
        DeleteObject(mask);
        DeleteObject(bitmap);
        DeleteDC(dc);
        icon
    }
}

unsafe fn update(hwnd: HWND) -> bool {
    unsafe {
        let state = UI.with(|ui| ui.borrow().as_ref().unwrap().state.lock().unwrap().clone());
        let size = icon_size(hwnd);
        let key = RenderKey {
            values: state.windows.each_ref().map(|window| {
                window
                    .as_ref()
                    .map(|window| window.remaining.round() as i32)
            }),
            stale: state.stale(),
            size,
        };
        let unchanged = UI.with(|ui| {
            let ui = ui.borrow();
            let ui = ui.as_ref().unwrap();
            ui.added && ui.last_render.as_ref() == Some(&key)
        });
        if unchanged {
            return true;
        }
        let icon = make_icon(&state, size);
        if icon.is_null() {
            return false;
        }
        let mut data: NOTIFYICONDATAW = zeroed();
        data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
        data.hWnd = hwnd;
        data.uID = 1;
        data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        data.uCallbackMessage = CALLBACK;
        data.hIcon = icon;
        let mut tip = String::from("Codex 剩余额度");
        for window in state.windows.iter().flatten() {
            tip.push_str(&format!(
                "\n{}：{:.0}%",
                window.description(),
                window.remaining
            ));
        }
        if state.stale() {
            tip.push_str(if state.updated.is_some() {
                "\n数据已过期"
            } else {
                "\n正在连接 / 暂无数据"
            });
        }
        tip.push_str("\n左键详情 · 右键菜单");
        for (dst, src) in data.szTip.iter_mut().take(127).zip(tip.encode_utf16()) {
            *dst = src;
        }
        let added = UI.with(|ui| ui.borrow().as_ref().unwrap().added);
        let ok = Shell_NotifyIconW(if added { NIM_MODIFY } else { NIM_ADD }, &data) != 0;
        UI.with(|ui| {
            let mut ui = ui.borrow_mut();
            let ui = ui.as_mut().unwrap();
            ui.added = ok;
            if ok {
                ui.last_render = Some(key);
            }
        });
        DestroyIcon(icon);
        if ok {
            // Icon creation loads comparatively large font/GDI pages. The tray
            // is idle almost all the time, so return those pages to Windows.
            crate::memory::current_process();
        }
        ok
    }
}

unsafe fn details(hwnd: HWND) {
    unsafe {
        // MessageBox runs a nested message loop, so tray callbacks can re-enter
        // this function before the first dialog has closed.
        let already_open = UI.with(|ui| {
            let mut ui = ui.borrow_mut();
            let ui = ui.as_mut().unwrap();
            let open = ui.details_open;
            ui.details_open = true;
            open
        });
        if already_open {
            let dialog = GetLastActivePopup(hwnd);
            if dialog != hwnd && IsWindowVisible(dialog) != 0 {
                SetForegroundWindow(dialog);
            }
            return;
        }
        let state = UI.with(|ui| ui.borrow().as_ref().unwrap().state.lock().unwrap().clone());
        let text = crate::usage_widget::details(&state)
            .replace("左键拖动位置 · 右键打开菜单", "托盘数字为短期剩余百分比");
        SetForegroundWindow(hwnd);
        MessageBoxW(
            hwnd,
            wide(&text).as_ptr(),
            wide("Codex 额度").as_ptr(),
            MB_OK,
        );
        UI.with(|ui| {
            if let Some(ui) = ui.borrow_mut().as_mut() {
                ui.details_open = false;
            }
        });
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        let restart = UI.with(|ui| ui.borrow().as_ref().map(|ui| ui.restart));
        if restart == Some(message) && message != 0 {
            UI.with(|ui| {
                let mut ui = ui.borrow_mut();
                let ui = ui.as_mut().unwrap();
                ui.added = false;
                ui.last_render = None;
            });
            update(hwnd);
            return 0;
        }
        match message {
            WM_TIMER | WM_DPICHANGED | WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
                update(hwnd);
                0
            }
            CALLBACK => {
                match l as u32 {
                    WM_LBUTTONUP => details(hwnd),
                    WM_RBUTTONUP | WM_CONTEXTMENU => {
                        let menu = CreatePopupMenu();
                        AppendMenuW(menu, MF_STRING, 1, wide("立即刷新").as_ptr());
                        AppendMenuW(menu, MF_STRING, 2, wide("额度详情").as_ptr());
                        AppendMenuW(menu, MF_SEPARATOR, 0, null());
                        AppendMenuW(menu, MF_STRING, 3, wide("退出额度显示").as_ptr());
                        let mut point: POINT = zeroed();
                        GetCursorPos(&mut point);
                        SetForegroundWindow(hwnd);
                        let choice = TrackPopupMenu(
                            menu,
                            TPM_RETURNCMD | TPM_RIGHTBUTTON,
                            point.x,
                            point.y,
                            0,
                            hwnd,
                            null(),
                        );
                        DestroyMenu(menu);
                        match choice {
                            1 => UI.with(|ui| {
                                let _ = ui
                                    .borrow()
                                    .as_ref()
                                    .unwrap()
                                    .action
                                    .send(usage::Action::Refresh);
                            }),
                            2 => details(hwnd),
                            3 => {
                                DestroyWindow(hwnd);
                            }
                            _ => {}
                        }
                        PostMessageW(hwnd, WM_NULL, 0, 0);
                    }
                    _ => {}
                }
                0
            }
            WM_DESTROY => {
                let mut data: NOTIFYICONDATAW = zeroed();
                data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
                data.hWnd = hwnd;
                data.uID = 1;
                Shell_NotifyIconW(NIM_DELETE, &data);
                KillTimer(hwnd, 1);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, w, l),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_native_bitmaps_for_common_dpi_scales() {
        let state = usage::State {
            windows: [
                Some(usage::Window {
                    remaining: 100.0,
                    minutes: 300,
                    resets_at: None,
                }),
                None,
            ],
            updated: Some(std::time::Instant::now()),
            error: None,
        };
        unsafe {
            for size in [16, 20, 24, 28, 32, 40, 48] {
                let icon = make_icon(&state, size);
                assert!(!icon.is_null());
                let mut info: ICONINFO = zeroed();
                assert_ne!(GetIconInfo(icon, &mut info), 0);
                let mut bitmap: BITMAP = zeroed();
                assert_ne!(
                    GetObjectW(
                        info.hbmColor,
                        size_of::<BITMAP>() as i32,
                        (&mut bitmap as *mut BITMAP).cast()
                    ),
                    0
                );
                assert_eq!((bitmap.bmWidth, bitmap.bmHeight), (size, size));
                DeleteObject(info.hbmColor);
                DeleteObject(info.hbmMask);
                DestroyIcon(icon);
            }
        }
    }
}
