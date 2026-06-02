use crate::upload::{REMOTE_LAUNCH, REMOTE_SHPOOL_PATH, REMOTE_SOCKET_PATH};

pub fn build_shpool_cmd(
    session: &str,
    shell: Option<&str>,
    remote_cwd: Option<&str>,
) -> String {
    let mut cmd = format!(
        "{REMOTE_SHPOOL_PATH} --socket {REMOTE_SOCKET_PATH} attach -f {session}"
    );
    match shell {
        Some(s) => cmd.push_str(&format!(r#" -c "{REMOTE_LAUNCH} {s}""#)),
        None => cmd.push_str(&format!(" -c {REMOTE_LAUNCH}")),
    }
    if let Some(cwd) = remote_cwd {
        cmd.push_str(&format!(" -d {}", shell_escape(cwd)));
    }
    cmd
}

fn shell_escape(s: &str) -> String {
    if s.contains(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == '\\') {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_shell() {
        let cmd = build_shpool_cmd("s0", None, None);
        assert!(cmd.contains("attach -f s0"));
        assert!(cmd.contains("-c $HOME/.local/share/sshr/init/launch.sh"));
        assert!(!cmd.contains("-d "));
    }

    #[test]
    fn test_shell_override_with_cwd() {
        let cmd = build_shpool_cmd("s0", Some("/bin/zsh"), Some("~/projects"));
        assert!(cmd.contains("attach -f s0"));
        assert!(cmd.contains(r#"-c "$HOME/.local/share/sshr/init/launch.sh /bin/zsh""#));
        assert!(cmd.contains("-d ~/projects"));
    }
}
