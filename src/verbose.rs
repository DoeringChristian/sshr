use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set(v: bool) {
    VERBOSE.store(v, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

#[macro_export]
macro_rules! vlog {
    ($($arg:tt)*) => {{
        if $crate::verbose::enabled() {
            use ::owo_colors::OwoColorize;
            eprintln!("{} {}", "sshr:".dimmed(), format!($($arg)*));
        }
    }};
}
