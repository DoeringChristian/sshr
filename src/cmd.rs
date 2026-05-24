use crate::probe::SessionTool;

const FISH_INIT: &str = "\
set -gx SSH_CONNECTION 1; \
function __sshr_osc7 --on-event fish_prompt; \
printf \\e]7\\;file://%s%s\\a (hostname) $PWD; \
end";

/// Build the remote command string to execute via SSH.
///
/// Returns `None` if no remote command is needed (plain SSH).
pub fn build_remote_cmd(
    tool: &SessionTool,
    session: &str,
    shell_path: Option<&str>,
    remote_cwd: Option<&str>,
) -> Option<String> {
    match tool {
        SessionTool::Shpool { path } => {
            let mut cmd = format!("{path} attach -f {session}");
            if let Some(shell) = shell_path {
                cmd.push_str(&format!(
                    " -c '{shell} -C \"{FISH_INIT}\"'"
                ));
            }
            if let Some(cwd) = remote_cwd {
                cmd.push_str(&format!(" -d {}", shell_escape(cwd)));
            }
            Some(cmd)
        }
        SessionTool::Abduco { path } => {
            let sh = shell_path.unwrap_or("\"$SHELL\"");
            if let Some(cwd) = remote_cwd {
                Some(format!(
                    "cd {} && {path} -A {session} {sh}",
                    shell_escape(cwd)
                ))
            } else {
                Some(format!("{path} -A {session} {sh}"))
            }
        }
        SessionTool::None => {
            match (shell_path, remote_cwd) {
                (Some(shell), Some(cwd)) => {
                    Some(format!(
                        "{shell} -C \"{FISH_INIT}; cd {}\"",
                        shell_escape(cwd)
                    ))
                }
                (Some(shell), None) => {
                    Some(format!("{shell} -C \"{FISH_INIT}\""))
                }
                (None, Some(cwd)) => {
                    Some(format!("cd {} && \"$SHELL\"", shell_escape(cwd)))
                }
                (None, None) => None,
            }
        }
    }
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
        let cmd = build_remote_cmd(
            &SessionTool::Shpool {
                path: "/home/u/.nix-profile/bin/shpool".into(),
            },
            "s0",
            Some("/home/u/.nix-profile/bin/fish"),
            Some("~/projects"),
        );
        let cmd = cmd.unwrap();
        assert!(cmd.contains("shpool attach -f s0"));
        assert!(cmd.contains("-c '/home/u/.nix-profile/bin/fish"));
        assert!(cmd.contains("SSH_CONNECTION"));
        assert!(cmd.contains("__sshr_osc7"));
        assert!(cmd.contains("-d ~/projects"));
    }

    #[test]
    fn test_shpool_no_shell_no_cwd() {
        let cmd = build_remote_cmd(
            &SessionTool::Shpool {
                path: "/usr/bin/shpool".into(),
            },
            "s1",
            None,
            None,
        );
        assert_eq!(cmd.unwrap(), "/usr/bin/shpool attach -f s1");
    }

    #[test]
    fn test_abduco_with_cwd() {
        let cmd = build_remote_cmd(
            &SessionTool::Abduco {
                path: "/usr/bin/abduco".into(),
            },
            "s0",
            Some("/usr/bin/fish"),
            Some("/tmp"),
        );
        assert_eq!(
            cmd.unwrap(),
            "cd /tmp && /usr/bin/abduco -A s0 /usr/bin/fish"
        );
    }

    #[test]
    fn test_none_plain_ssh() {
        let cmd = build_remote_cmd(&SessionTool::None, "", None, None);
        assert!(cmd.is_none());
    }

    #[test]
    fn test_none_with_shell_only() {
        let cmd = build_remote_cmd(&SessionTool::None, "", Some("/usr/bin/fish"), None);
        let cmd = cmd.unwrap();
        assert!(cmd.starts_with("/usr/bin/fish -C"));
        assert!(cmd.contains("__sshr_osc7"));
    }
}
