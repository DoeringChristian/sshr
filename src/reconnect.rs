use anyhow::Result;
use owo_colors::OwoColorize;
use std::io::Read;
use std::process::ExitStatus;

/// Run a connection function in a loop, prompting to reconnect on failure.
pub fn run_with_reconnect<F>(connect: F) -> Result<()>
where
    F: Fn() -> Result<ExitStatus>,
{
    loop {
        let status = connect()?;

        if status.success() {
            break;
        }

        eprintln!();
        eprintln!(
            "{}",
            "Connection lost. Press any key to reconnect (Ctrl-C to quit)..."
                .yellow()
                .bold()
        );

        if !wait_for_keypress() {
            break;
        }

        eprintln!("{}", "Reconnecting...".dimmed());
    }

    Ok(())
}

/// Wait for a single keypress. Returns false on EOF or error (e.g. Ctrl-C).
fn wait_for_keypress() -> bool {
    let mut buf = [0u8; 1];
    matches!(std::io::stdin().read(&mut buf), Ok(1..))
}
