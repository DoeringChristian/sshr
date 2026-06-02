use crate::upload::{REMOTE_SHPOOL_PATH, REMOTE_SOCKET_PATH};

const FISH_INIT: &str = "\
set -gx SSH_CONNECTION 1; \
function __sshr_osc7 --on-event fish_prompt; \
printf \\e]7\\;file://%s%s\\a (hostname) $PWD; \
end";

pub fn build_shpool_cmd(
    session: &str,
    shell_path: Option<&str>,
    remote_cwd: Option<&str>,
) -> String {
    let mut cmd = format!(
        "{REMOTE_SHPOOL_PATH} --socket {REMOTE_SOCKET_PATH} attach -f {session}"
    );
    if let Some(shell) = shell_path {
        cmd.push_str(&format!(" -c '{shell} -C \"{FISH_INIT}\"'"));
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
    fn test_shpool_with_fish_and_cwd() {
        let cmd = build_shpool_cmd(
            "s0",
            Some("/usr/bin/fish"),
            Some("~/projects"),
        );
        assert!(cmd.contains("--socket"));
        assert!(cmd.contains("attach -f s0"));
        assert!(cmd.contains("-c '/usr/bin/fish"));
        assert!(cmd.contains("SSH_CONNECTION"));
        assert!(cmd.contains("__sshr_osc7"));
        assert!(cmd.contains("-d ~/projects"));
    }

    #[test]
    fn test_shpool_no_shell_no_cwd() {
        let cmd = build_shpool_cmd("s1", None, None);
        assert!(cmd.contains("--socket"));
        assert!(cmd.contains("attach -f s1"));
        assert!(!cmd.contains("-c "));
        assert!(!cmd.contains("-d "));
    }

}
