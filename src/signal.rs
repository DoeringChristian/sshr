use std::os::unix::ffi::OsStringExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static CLOSING: AtomicBool = AtomicBool::new(false);
static WAL_CONTEXT: Mutex<Option<WalContext>> = Mutex::new(None);

struct WalContext {
    wal_path: Vec<u8>,
    entry_line: Vec<u8>,
}

extern "C" fn handle_signal(_: libc::c_int) {
    CLOSING.store(true, Ordering::SeqCst);

    // Write WAL entry using async-signal-safe syscalls
    if let Ok(guard) = WAL_CONTEXT.try_lock() {
        if let Some(ctx) = guard.as_ref() {
            unsafe {
                let fd = libc::open(
                    ctx.wal_path.as_ptr() as *const libc::c_char,
                    libc::O_WRONLY | libc::O_CREAT | libc::O_APPEND,
                    0o644,
                );
                if fd >= 0 {
                    libc::write(fd, ctx.entry_line.as_ptr() as *const _, ctx.entry_line.len());
                    libc::close(fd);
                }
            }
        }
    }
}

pub fn install_handlers(host: &str, session_name: &str) {
    let wal_path = crate::wal::wal_path();

    // Ensure the WAL directory exists before we need it in a signal handler
    if let Some(parent) = wal_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Null-terminated path for libc::open
    let mut path_bytes = wal_path.into_os_string().into_vec();
    path_bytes.push(0);

    let entry_line = format!("{host}:{session_name}\n").into_bytes();

    *WAL_CONTEXT.lock().unwrap() = Some(WalContext {
        wal_path: path_bytes,
        entry_line,
    });

    unsafe {
        libc::signal(libc::SIGHUP, handle_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, handle_signal as *const () as libc::sighandler_t);
    }
}

pub fn is_closing() -> bool {
    CLOSING.load(Ordering::SeqCst)
}

#[cfg(test)]
#[allow(dead_code)]
pub fn set_closing(val: bool) {
    CLOSING.store(val, Ordering::SeqCst);
}
