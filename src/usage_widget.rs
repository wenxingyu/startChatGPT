//! A small taskbar-style top-level window; does not inject into Explorer.
use crate::{config::ProxySetting, usage};
use std::cell::RefCell;
use std::mem::zeroed;
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::{Arc, Mutex, mpsc::Sender};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::*,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};

struct Ui {
    state: Arc<Mutex<usage::State>>,
    action: Sender<usage::Action>,
    scale: f64,
    docked: bool,
}
thread_local! { static UI: RefCell<Option<Ui>> = const { RefCell::new(None) }; }
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}

pub fn run(app: &Path, proxy: ProxySetting) -> Result<(), String> {
    unsafe {
        let class = wide("StartChatGPTQuotaWidget");
        let existing = FindWindowW(class.as_ptr(), null());
        if !existing.is_null() {
            ShowWindow(existing, SW_SHOWNOACTIVATE);
            return Ok(());
        }
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_SIZEALL),
            ..zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return Err("无法注册额度窗口".into());
        }
        let scale = GetDpiForSystem() as f64 / 96.0;
        let state = Arc::new(Mutex::new(usage::State::default()));
        let (action, worker) = usage::start(app.to_owned(), proxy, state.clone());
        UI.with(|ui| {
            *ui.borrow_mut() = Some(Ui {
                state,
                action: action.clone(),
                scale,
                docked: true,
            })
        });
        // Optional UI inspection mode exposes the same rendered panel to window automation.
        let inspect = std::env::var_os("STARTCHATGPT_USAGE_INSPECT").is_some();
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST
                | if inspect {
                    WS_EX_APPWINDOW
                } else {
                    WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE
                },
            class.as_ptr(),
            wide("Codex 额度 · 剩余").as_ptr(),
            WS_POPUP,
            0,
            0,
            (174.0 * scale) as i32,
            (46.0 * scale) as i32,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if !hwnd.is_null() {
            position(hwnd);
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            SetTimer(hwnd, 1, 1000, None);
            let mut message: MSG = zeroed();
            while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        let _ = action.send(usage::Action::Stop);
        UI.with(|ui| *ui.borrow_mut() = None);
        // The worker owns the child process and always kills/waits it on shutdown.
        let _ = worker.join();
        if hwnd.is_null() {
            Err("无法创建额度窗口".into())
        } else {
            Ok(())
        }
    }
}

unsafe fn position(hwnd: HWND) {
    unsafe {
        let taskbar = FindWindowW(wide("Shell_TrayWnd").as_ptr(), null());
        let mut bar: RECT = zeroed();
        let mut scale = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
        let (x, y, width, height) = if !taskbar.is_null() && GetWindowRect(taskbar, &mut bar) != 0 {
            let horizontal = bar.right - bar.left > bar.bottom - bar.top;
            // Fit the two rows even when Windows is configured with a short taskbar.
            if horizontal {
                scale = scale.min(((bar.bottom - bar.top - 2).max(16)) as f64 / 46.0);
            }
            let width = (174.0 * scale).round() as i32;
            let height = (46.0 * scale).round() as i32;
            let gap = (6.0 * scale).round() as i32;
            let tray = FindWindowExW(taskbar, null_mut(), wide("TrayNotifyWnd").as_ptr(), null());
            let mut notify: RECT = zeroed();
            let right = if !tray.is_null() && GetWindowRect(tray, &mut notify) != 0 {
                notify.left
            } else {
                bar.right - (230.0 * scale) as i32
            };
            if horizontal {
                (
                    (right - width - gap).max(bar.left),
                    bar.top + (bar.bottom - bar.top - height) / 2,
                    width,
                    height,
                )
            } else {
                (bar.right + gap, bar.bottom - height - gap, width, height)
            }
        } else {
            let width = (174.0 * scale).round() as i32;
            let height = (46.0 * scale).round() as i32;
            (
                GetSystemMetrics(SM_CXSCREEN) - width - 16,
                GetSystemMetrics(SM_CYSCREEN) - height - 64,
                width,
                height,
            )
        };
        UI.with(|ui| ui.borrow_mut().as_mut().unwrap().scale = scale);
        SetWindowPos(hwnd, HWND_TOPMOST, x, y, width, height, SWP_NOACTIVATE);
    }
}

unsafe fn text(dc: HDC, value: &str, rect: RECT, color: u32, size: i32, bold: bool, align: u32) {
    unsafe {
        let font = CreateFontW(
            -size,
            0,
            0,
            0,
            if bold { 600 } else { 400 },
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Segoe UI").as_ptr(),
        );
        let old = SelectObject(dc, font);
        SetTextColor(dc, color);
        SetBkMode(dc, TRANSPARENT as i32);
        let mut rect = rect;
        DrawTextW(
            dc,
            wide(value).as_ptr(),
            -1,
            &mut rect,
            DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX | align,
        );
        SelectObject(dc, old);
        DeleteObject(font);
    }
}

unsafe fn paint(hwnd: HWND) {
    unsafe {
        let mut ps: PAINTSTRUCT = zeroed();
        let target = BeginPaint(hwnd, &mut ps);
        let mut bounds: RECT = zeroed();
        GetClientRect(hwnd, &mut bounds);
        // Double-buffer to avoid flashing during the one-second stale-state check.
        let dc = CreateCompatibleDC(target);
        let bitmap = CreateCompatibleBitmap(target, bounds.right, bounds.bottom);
        let old_bitmap = SelectObject(dc, bitmap);
        let background = CreateSolidBrush(rgb(28, 31, 36));
        FillRect(dc, &bounds, background);
        DeleteObject(background);
        UI.with(|ui| {
            let ui = ui.borrow();
            let ui = ui.as_ref().unwrap();
            let state = ui.state.lock().unwrap().clone();
            let s = |n: i32| (n as f64 * ui.scale).round() as i32;
            let stale = state.stale();
            let status = if stale {
                rgb(154, 163, 175)
            } else {
                rgb(102, 220, 170)
            };
            let accent = CreateSolidBrush(status);
            FillRect(
                dc,
                &RECT {
                    left: 0,
                    top: s(4),
                    right: s(3),
                    bottom: bounds.bottom - s(4),
                },
                accent,
            );
            DeleteObject(accent);
            text(
                dc,
                "CODEX",
                RECT {
                    left: s(11),
                    top: 0,
                    right: s(60),
                    bottom: s(24),
                },
                rgb(214, 220, 229),
                s(10),
                true,
                DT_LEFT,
            );
            text(
                dc,
                if state.updated.is_none() {
                    if state.error.is_some() {
                        "未连接"
                    } else {
                        "连接中"
                    }
                } else if stale {
                    "已过期"
                } else {
                    "剩余"
                },
                RECT {
                    left: s(11),
                    top: s(23),
                    right: s(60),
                    bottom: bounds.bottom,
                },
                status,
                s(10),
                false,
                DT_LEFT,
            );
            for (i, window) in state.windows.iter().enumerate() {
                let label = window.as_ref().map_or_else(
                    || if i == 0 { "5H".into() } else { "1W".into() },
                    usage::Window::label,
                );
                let value = window
                    .as_ref()
                    .map_or("--".into(), |w| format!("{:.0}%", w.remaining));
                let color = if stale {
                    rgb(154, 163, 175)
                } else if window.as_ref().is_some_and(|w| w.remaining <= 10.0) {
                    rgb(255, 117, 126)
                } else if window.as_ref().is_some_and(|w| w.remaining <= 25.0) {
                    rgb(246, 195, 94)
                } else {
                    rgb(102, 220, 170)
                };
                let top = s(2 + i as i32 * 21);
                text(
                    dc,
                    &label,
                    RECT {
                        left: s(68),
                        top,
                        right: s(101),
                        bottom: top + s(21),
                    },
                    rgb(181, 191, 206),
                    s(12),
                    false,
                    DT_LEFT,
                );
                text(
                    dc,
                    &value,
                    RECT {
                        left: s(102),
                        top,
                        right: bounds.right - s(10),
                        bottom: top + s(21),
                    },
                    color,
                    s(17),
                    true,
                    DT_RIGHT,
                );
            }
        });
        BitBlt(target, 0, 0, bounds.right, bounds.bottom, dc, 0, 0, SRCCOPY);
        SelectObject(dc, old_bitmap);
        DeleteObject(bitmap);
        DeleteDC(dc);
        EndPaint(hwnd, &ps);
    }
}

pub(crate) fn details(state: &usage::State) -> String {
    let mut lines = vec!["Codex 账户额度（剩余）".to_string()];
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    for window in state.windows.iter().flatten() {
        let reset = window
            .resets_at
            .map(|t| {
                let mins = t.saturating_sub(now).div_ceil(60);
                format!("，约 {} 小时 {} 分钟后重置", mins / 60, mins % 60)
            })
            .unwrap_or_default();
        lines.push(format!(
            "{}：{:.0}%{}",
            window.description(),
            window.remaining,
            reset
        ));
    }
    if let Some(t) = state.updated {
        lines.push(format!("上次成功更新：{} 秒前", t.elapsed().as_secs()));
    }
    if let Some(error) = &state.error {
        lines.push(error.clone());
    }
    lines.push("每 45 秒刷新 · 左键拖动位置 · 右键打开菜单".into());
    lines.join("\n")
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_PAINT => {
                paint(hwnd);
                0
            }
            WM_ERASEBKGND => 1,
            WM_TIMER => {
                if UI.with(|ui| ui.borrow().as_ref().is_some_and(|ui| ui.docked)) {
                    position(hwnd);
                }
                InvalidateRect(hwnd, null(), 0);
                let state =
                    UI.with(|ui| ui.borrow().as_ref().unwrap().state.lock().unwrap().clone());
                SetWindowTextW(hwnd, wide(&details(&state)).as_ptr());
                0
            }
            WM_LBUTTONDOWN => {
                UI.with(|ui| ui.borrow_mut().as_mut().unwrap().docked = false);
                SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
                0
            }
            WM_RBUTTONUP => {
                let menu = CreatePopupMenu();
                AppendMenuW(menu, MF_STRING, 1, wide("立即刷新").as_ptr());
                AppendMenuW(menu, MF_STRING, 2, wide("额度详情").as_ptr());
                AppendMenuW(menu, MF_STRING, 3, wide("停靠到时间区域左侧").as_ptr());
                AppendMenuW(menu, MF_SEPARATOR, 0, null());
                AppendMenuW(menu, MF_STRING, 4, wide("退出额度显示").as_ptr());
                let mut cursor: POINT = zeroed();
                GetCursorPos(&mut cursor);
                SetForegroundWindow(hwnd);
                let choice = TrackPopupMenu(
                    menu,
                    TPM_RETURNCMD | TPM_RIGHTBUTTON,
                    cursor.x,
                    cursor.y,
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
                    2 => {
                        let state = UI
                            .with(|ui| ui.borrow().as_ref().unwrap().state.lock().unwrap().clone());
                        MessageBoxW(
                            hwnd,
                            wide(&details(&state)).as_ptr(),
                            wide("Codex 额度").as_ptr(),
                            MB_OK,
                        );
                    }
                    3 => {
                        UI.with(|ui| ui.borrow_mut().as_mut().unwrap().docked = true);
                        position(hwnd);
                    }
                    4 => {
                        DestroyWindow(hwnd);
                    }
                    _ => {}
                }
                PostMessageW(hwnd, WM_NULL, 0, 0);
                0
            }
            WM_DPICHANGED => {
                UI.with(|ui| ui.borrow_mut().as_mut().unwrap().scale = (w & 0xffff) as f64 / 96.0);
                let rect = &*(l as *const RECT);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                0
            }
            WM_DISPLAYCHANGE | WM_SETTINGCHANGE => {
                if UI.with(|ui| ui.borrow().as_ref().is_some_and(|ui| ui.docked)) {
                    position(hwnd);
                }
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, 1);
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, w, l),
        }
    }
}
