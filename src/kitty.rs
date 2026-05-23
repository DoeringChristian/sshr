use base64::Engine;

/// Check if we're running inside Kitty terminal.
pub fn is_kitty() -> bool {
    std::env::var("KITTY_WINDOW_ID").is_ok()
}

/// Set a Kitty user variable via OSC 1337 escape sequence.
pub fn set_user_var(key: &str, value: &str) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(value);
    eprint!("\x1b]1337;SetUserVar={key}={encoded}\x07");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_kitty_unset() {
        // In test env, KITTY_WINDOW_ID is not set
        std::env::remove_var("KITTY_WINDOW_ID");
        assert!(!is_kitty());
    }
}
