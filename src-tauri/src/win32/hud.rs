use windows::{
    core::Error,
    Win32::{
        Foundation::{HWND, POINT, RECT},
        UI::WindowsAndMessaging::{
            GetCursorPos, GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
        },
    },
};

pub fn cursor_x() -> Result<i32, String> {
    let mut cursor = POINT::default();
    // The point storage is writable for the duration of GetCursorPos.
    unsafe { GetCursorPos(&mut cursor) }
        .map_err(|error| format!("Could not read the pointer position: {error}"))?;
    Ok(cursor.x)
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
