use crate::OpenWindow;
use std::{ffi::c_void, path::Path};
use windows::{
    core::{Error, Result, BOOL, PWSTR},
    Win32::{
        Foundation::{CloseHandle, GetLastError, SetLastError, HWND, LPARAM, RECT, WIN32_ERROR},
        Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED},
        System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowLongW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
            GetWindowThreadProcessId, IsWindowVisible, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
        },
    },
};

pub(crate) fn list_open_windows() -> Result<Vec<OpenWindow>> {
    let mut windows: Vec<OpenWindow> = Vec::new();
    // EnumWindows calls back synchronously on this thread and never retains the pointer.
    unsafe {
        EnumWindows(
            Some(collect_window),
            LPARAM(std::ptr::from_mut(&mut windows) as isize),
        )?
    };
    windows.sort_by(|a, b| {
        a.process_name
            .to_lowercase()
            .cmp(&b.process_name.to_lowercase())
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            .then_with(|| a.hwnd.cmp(&b.hwnd))
    });
    Ok(windows)
}

unsafe extern "system" fn collect_window(hwnd: HWND, context: LPARAM) -> BOOL {
    match inspect_window(hwnd) {
        Ok(Some(window)) => {
            // This pointer is the exclusive accumulator passed to EnumWindows above.
            unsafe { &mut *(context.0 as *mut Vec<OpenWindow>) }.push(window);
        }
        Ok(None) => {}
        Err(error) => eprintln!("Could not inspect window {:?}: {error}", hwnd.0),
    }
    true.into()
}

fn inspect_window(hwnd: HWND) -> Result<Option<OpenWindow>> {
    // HWND is an opaque OS handle. Each out parameter is backed by writable storage.
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return Ok(None);
        }

        let mut pid = 0;
        SetLastError(WIN32_ERROR(0));
        if GetWindowThreadProcessId(hwnd, Some(&mut pid)) == 0 {
            return Err(Error::from_thread());
        }
        if pid == std::process::id() {
            return Ok(None);
        }

        let mut cloaked = 0u32;
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&mut cloaked as *mut u32).cast::<c_void>(),
            std::mem::size_of_val(&cloaked) as u32,
        )?;
        if cloaked != 0 {
            return Ok(None);
        }

        let mut rect = RECT::default();
        GetWindowRect(hwnd, &mut rect)?;
        if rect.right <= rect.left || rect.bottom <= rect.top {
            return Ok(None);
        }

        SetLastError(WIN32_ERROR(0));
        let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if style == 0 && GetLastError().0 != 0 {
            return Err(Error::from_thread());
        }
        if style & WS_EX_TOOLWINDOW.0 != 0 {
            return Ok(None);
        }

        let title = window_title(hwnd)?;
        if title.trim().is_empty() {
            return Ok(None);
        }

        let process_name = executable_name(pid).unwrap_or_else(|error| {
            eprintln!("Could not read the executable for process {pid}: {error}");
            format!("Process {pid}")
        });
        Ok(Some(OpenWindow {
            hwnd: hwnd.0 as isize,
            title,
            process_name,
            pid,
        }))
    }
}

pub(super) fn window_title(hwnd: HWND) -> Result<String> {
    // Read caption text using a buffer sized for the indicated UTF-16 length.
    unsafe {
        SetLastError(WIN32_ERROR(0));
        let title_length = GetWindowTextLengthW(hwnd);
        if title_length == 0 {
            return if GetLastError().0 == 0 {
                Ok(String::new())
            } else {
                Err(Error::from_thread())
            };
        }
        let mut buffer = vec![0u16; title_length as usize + 1];
        SetLastError(WIN32_ERROR(0));
        let written = GetWindowTextW(hwnd, &mut buffer) as usize;
        if written == 0 && GetLastError().0 != 0 {
            return Err(Error::from_thread());
        }
        Ok(String::from_utf16_lossy(&buffer[..written]))
    }
}

pub(super) fn executable_name(pid: u32) -> Result<String> {
    let mut buffer = vec![0u16; 32_768];
    let mut length = buffer.len() as u32;
    // The buffer and its length match. Close the owned handle on both query outcomes.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)?;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        );
        if let Err(error) = CloseHandle(handle) {
            eprintln!("Could not close the handle for process {pid}: {error}");
        }
        result?;
    }
    let path = String::from_utf16_lossy(&buffer[..length as usize]);
    Ok(Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&path)
        .to_owned())
}
