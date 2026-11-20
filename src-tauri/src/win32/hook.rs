use std::{
    cell::RefCell,
    sync::mpsc,
    thread::{self, JoinHandle},
};
use windows::{
    core::Error,
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        System::Threading::GetCurrentThreadId,
        UI::{
            Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK},
            WindowsAndMessaging::{
                DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW, TranslateMessage,
                EVENT_SYSTEM_FOREGROUND, MSG, PM_NOREMOVE, WINEVENT_OUTOFCONTEXT,
                WINEVENT_SKIPOWNPROCESS, WM_QUIT,
            },
        },
    },
};

type Callback = Box<dyn Fn(HookEvent) + Send>;

thread_local! {
    static CALLBACK: RefCell<Option<Callback>> = RefCell::new(None);
}

pub enum HookEvent {
    Foreground(isize),
    Failed(String),
}

pub struct ForegroundHook {
    thread_id: u32,
    worker: Option<JoinHandle<()>>,
}

impl ForegroundHook {
    pub fn start(callback: impl Fn(HookEvent) + Send + 'static) -> Result<Self, String> {
        let (ready, started) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("anchor-foreground".into())
            .spawn(move || worker(Box::new(callback), ready))
            .map_err(|error| format!("Could not start the Windows foreground worker: {error}"))?;
        match started.recv() {
            Ok(Ok(thread_id)) => Ok(Self {
                thread_id,
                worker: Some(worker),
            }),
            outcome => {
                if worker.join().is_err() {
                    eprintln!("Foreground worker panicked during startup");
                }
                Err(match outcome {
                    Ok(Err(error)) => error,
                    _ => "The Windows foreground worker exited during startup.".into(),
                })
            }
        }
    }
}

impl Drop for ForegroundHook {
    fn drop(&mut self) {
        if let Some(worker) = self.worker.take() {
            // The worker created its message queue before sending the thread ID.
            if !worker.is_finished() {
                if let Err(error) =
                    unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }
                {
                    if !worker.is_finished() {
                        eprintln!("Could not post WM_QUIT to the foreground worker: {error}");
                    }
                }
            }
            if worker.join().is_err() {
                eprintln!("Foreground worker panicked");
            }
        }
    }
}

fn worker(callback: Callback, ready: mpsc::SyncSender<Result<u32, String>>) {
    CALLBACK.with(|slot| slot.replace(Some(callback)));
    let mut message = MSG::default();
    // Out-of-context callbacks are delivered on this thread while it pumps messages.
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
        let handle = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(on_foreground),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if handle.0.is_null() {
            let error = format!(
                "Could not install the Windows foreground hook: {}",
                Error::from_thread()
            );
            if ready.send(Err(error)).is_err() {
                eprintln!("Foreground hook startup receiver closed");
            }
            return;
        }

        if ready.send(Ok(GetCurrentThreadId())).is_ok() {
            loop {
                match GetMessageW(&mut message, None, 0, 0).0 {
                    0 => break,
                    -1 => {
                        notify(HookEvent::Failed(format!(
                            "Windows foreground message pump failed: {}",
                            Error::from_thread()
                        )));
                        break;
                    }
                    _ => {
                        let _ = TranslateMessage(&message);
                        DispatchMessageW(&message);
                    }
                }
            }
        }
        if !UnhookWinEvent(handle).as_bool() {
            eprintln!(
                "Could not unhook Windows foreground events: {}",
                Error::from_thread()
            );
        }
    }
}

fn notify(event: HookEvent) {
    CALLBACK.with(|slot| {
        if let Some(callback) = slot.borrow().as_ref() {
            callback(event);
        }
    });
}

unsafe extern "system" fn on_foreground(
    _: HWINEVENTHOOK,
    _: u32,
    hwnd: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    if !hwnd.0.is_null() {
        notify(HookEvent::Foreground(hwnd.0 as isize));
    }
}
