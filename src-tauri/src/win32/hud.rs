use std::ffi::c_void;
use windows::{
    core::Error,
    Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE,
            DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
        },
        UI::WindowsAndMessaging::{
            GetCursorPos, GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
        },
    },
};

pub fn strip_frame(raw: isize) {
    let hwnd = HWND(raw as *mut _);
    let no_border = DWMWA_COLOR_NONE;
    let no_round = DWMWCP_DONOTROUND;
    // Ignore errors on Windows 10 where these Windows 11 DWM attributes are unavailable.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            std::ptr::from_ref(&no_border) as *const c_void,
            std::mem::size_of_val(&no_border) as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            std::ptr::from_ref(&no_round) as *const c_void,
            std::mem::size_of_val(&no_round) as u32,
        );
    }
}

pub fn cursor_pos() -> Result<(i32, i32), String> {
    let mut cursor = POINT::default();
    // The point storage is writable for the duration of GetCursorPos.
    unsafe { GetCursorPos(&mut cursor) }
        .map_err(|error| format!("Could not read the pointer position: {error}"))?;
    Ok((cursor.x, cursor.y))
}

pub fn rect(raw: isize) -> Result<RECT, String> {
    let mut rect = RECT::default();
    // This is Anchor's own HUD HWND and rect is a writable out parameter.
    unsafe { GetWindowRect(HWND(raw as *mut _), &mut rect) }
        .map_err(|error| format!("Could not measure the HUD: {error}"))?;
    Ok(rect)
}

pub fn move_to(raw: isize, x: i32, y: i32) -> Result<(), String> {
    // Keep the existing z-order and let Windows apply DPI-dependent sizing.
    unsafe {
        SetWindowPos(
            HWND(raw as *mut _),
            None,
            x,
            y,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
        )
    }
    .map_err(|error: Error| format!("Could not move the HUD: {error}"))
}
