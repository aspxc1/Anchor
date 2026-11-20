use windows::{
    core::{w, PWSTR},
    Win32::{
        Foundation::HWND,
        UI::Controls::Dialogs::{
            CommDlgExtendedError, GetOpenFileNameW, OFN_DONTADDTORECENT, OFN_EXPLORER,
            OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
        },
    },
};

pub fn pick_executable(owner: isize) -> Result<Option<String>, String> {
    let mut filename = vec![0u16; 32_768];
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(owner as *mut _),
        lpstrFilter: w!("Windows programs (*.exe)\0*.exe\0\0"),
        lpstrFile: PWSTR(filename.as_mut_ptr()),
        nMaxFile: filename.len() as u32,
        lpstrTitle: w!("Choose a program for this study profile"),
        Flags: OFN_EXPLORER
            | OFN_FILEMUSTEXIST
            | OFN_PATHMUSTEXIST
            | OFN_NOCHANGEDIR
            | OFN_DONTADDTORECENT,
        ..Default::default()
    };
    // The string constants and writable filename buffer live through the modal call.
    if unsafe { GetOpenFileNameW(&mut dialog) }.as_bool() {
        let end = filename
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(filename.len());
        return String::from_utf16(&filename[..end]).map(Some).map_err(|_| {
            "Windows returned an executable path that Anchor could not decode.".into()
        });
    }
    // Cancellation is reported as 0; other values describe a real dialog error.
    let error = unsafe { CommDlgExtendedError() }.0;
    if error == 0 {
        Ok(None)
    } else {
        Err(format!(
            "Could not open the executable picker (Windows error {error:#06x})."
        ))
    }
}
