use std::sync::atomic::{AtomicBool, Ordering};

static CLOSING: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_signal(_: libc::c_int) {
    CLOSING.store(true, Ordering::SeqCst);
}

pub fn install_handlers() {
    unsafe {
        libc::signal(libc::SIGHUP, handle_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, handle_signal as *const () as libc::sighandler_t);
    }
}

pub fn is_closing() -> bool {
    CLOSING.load(Ordering::SeqCst)
}
