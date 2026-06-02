use anyhow::{Context, Result};

use crate::ssh::SshContext;
use crate::vlog;

const PROBE_SCRIPT: &str = r#"
find_cmd() {
    for tool in $@; do
        for dir in "$HOME/.local/share/sshr/bin" $(echo "$PATH" | tr ":" " ") "$HOME/.nix-profile/bin" "$HOME/.local/bin"; do
            if [ -x "$dir/$tool" ]; then
                echo "$dir/$tool"
                return
            fi
        done
    done
    echo none
}
find_cmd fish
"#;

pub fn probe_remote(ssh: &SshContext, host: &str, extra_args: &[String]) -> Result<Option<String>> {
    let output = ssh
        .run_capture(host, extra_args, PROBE_SCRIPT)
        .context("failed to probe remote")?;
    let fish_path = output.lines().next().unwrap_or("none").trim();
    if fish_path == "none" {
        vlog!("probe: fish not found on {host}");
        Ok(None)
    } else {
        vlog!("probe: fish = {fish_path}");
        Ok(Some(fish_path.to_string()))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_fish_found() {
        // Simulate probe output
        let output = "/home/user/.nix-profile/bin/fish\n";
        let fish_path = output.lines().next().unwrap_or("none").trim();
        assert_eq!(fish_path, "/home/user/.nix-profile/bin/fish");
    }

    #[test]
    fn test_parse_no_fish() {
        let output = "none\n";
        let fish_path = output.lines().next().unwrap_or("none").trim();
        assert_eq!(fish_path, "none");
    }
}
