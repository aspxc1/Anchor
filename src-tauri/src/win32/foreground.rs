use super::discovery::{executable_name, window_title};
use crate::policy::WindowId;
use windows::{
    core::Error,
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        UI::WindowsAndMessaging::{
            GetAncestor, GetClassNameW, GetForegroundWindow, GetWindow, GetWindowThreadProcessId,
            IsIconic, IsWindow, SetForegroundWindow, SetWindowPos, ShowWindow, GA_ROOTOWNER,
            GW_OWNER, HWND_TOPMOST, SWP_NOACTIVATE, SW_RESTORE,
        },
    },
};

fn handle(raw: isize) -> HWND {
    HWND(raw as *mut _)
}

pub fn current() -> isize {
    // GetForegroundWindow returns a borrowed opaque handle, or null during a switch.
    unsafe { GetForegroundWindow().0 as isize }
}

pub fn identify(raw: isize) -> Option<WindowId> {
    let mut pid = 0;
    // A foreground HWND can disappear before its queued event is processed.
    unsafe {
        if raw == 0 || !IsWindow(Some(handle(raw))).as_bool() {
            return None;
        }
        if GetWindowThreadProcessId(handle(raw), Some(&mut pid)) == 0 {
            eprintln!("Could not identify window {raw}: {}", Error::from_thread());
            return None;
        }
    }
    Some(WindowId { hwnd: raw, pid })
}

pub fn owners(raw: isize) -> Vec<WindowId> {
    // A missing owner is normal. Neither query gives us ownership of the returned handle.
    unsafe {
        let direct = GetWindow(handle(raw), GW_OWNER).ok();
        let root = GetAncestor(handle(raw), GA_ROOTOWNER);
        direct
            .into_iter()
            .chain([root])
            .filter_map(|owner| identify(owner.0 as isize))
            .collect()
    }
}

pub fn class_name(id: WindowId) -> String {
    let mut class = [0u16; 256];
    // GetClassNameW writes only within the supplied buffer.
    let len = unsafe { GetClassNameW(handle(id.hwnd), &mut class) } as usize;
    if len == 0 {
        eprintln!(
            "Could not classify foreground window {}: {}",
            id.hwnd,
            Error::from_thread()
        );
    }
    String::from_utf16_lossy(&class[..len])
}

pub fn shell_class(class: &str) -> bool {
    matches!(
        class,
        "Shell_TrayWnd"
            | "Shell_SecondaryTrayWnd"
            | "NotifyIconOverflowWindow"
            | "Progman"
            | "WorkerW"
    )
}

pub fn describe(id: WindowId) -> (String, String) {
    let executable = executable_name(id.pid).unwrap_or_else(|error| {
        eprintln!("Could not query foreground process {}: {error}", id.pid);
        format!("Process {}", id.pid)
    });
    let title = window_title(handle(id.hwnd)).unwrap_or_else(|error| {
        eprintln!("Could not read the foreground window title: {error}");
        String::new()
    });
    (executable, title)
}

pub fn activate(id: WindowId) -> Result<(), String> {
    if identify(id.hwnd) != Some(id) {
        return Err("That window has closed. Choose another window or unlock the session.".into());
    }
    let window = handle(id.hwnd);
    // ShowWindow returns the previous visibility, so its result is not a success flag.
    unsafe {
        if IsIconic(window).as_bool() {
            let _ = ShowWindow(window, SW_RESTORE);
        }
        if SetForegroundWindow(window).as_bool() {
            Ok(())
        } else {
            Err("Windows refused to activate the selected app. Click Return to Focus again.".into())
        }
    }
}

pub fn monitor_rect(raw: isize, work_area: bool) -> Result<RECT, String> {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // MonitorFromWindow supplies a borrowed monitor handle. MONITORINFO has the required size.
    unsafe {
        let monitor = MonitorFromWindow(handle(raw), MONITOR_DEFAULTTONEAREST);
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return Err(format!(
                "Could not locate the foreground window's monitor: {}",
                Error::from_thread()
            ));
        }
    }
    Ok(if work_area {
        info.rcWork
    } else {
        info.rcMonitor
    })
}

pub fn position_overlay(raw: isize, target: isize) -> Result<(), String> {
    let rect = monitor_rect(target, false)?;
    let window = handle(raw);
    // Position Anchor's own HWND in physical pixels without changing visibility.
    unsafe {
        SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOACTIVATE,
        )
        .map_err(|error| format!("Could not position the focus curtain: {error}"))?;
    }
    Ok(())
}

pub fn focus_overlay(raw: isize) {
    // The HWND belongs to the visible Anchor curtain.
    if !unsafe { SetForegroundWindow(handle(raw)) }.as_bool() {
        eprintln!("Windows declined direct foreground activation for the curtain");
    }
}
