use anyhow::{Context, Result};

use crate::ssh::SshContext;

#[derive(Debug, Clone)]
pub enum SessionTool {
    Shpool { path: String },
    Abduco { path: String },
    None,
}

impl SessionTool {
    pub fn name(&self) -> &str {
        match self {
            Self::Shpool { .. } => "shpool",
            Self::Abduco { .. } => "abduco",
            Self::None => "none",
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Self::Shpool { path } | Self::Abduco { path } => path,
            Self::None => "",
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub tool: SessionTool,
    pub fish_path: Option<String>,
}

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
find_cmd shpool abduco
find_cmd fish
"#;

pub fn probe_remote(ssh: &SshContext, host: &str, extra_args: &[String]) -> Result<ProbeResult> {
    let output = ssh
        .run_capture(host, extra_args, PROBE_SCRIPT)
        .context("failed to probe remote")?;
    parse_probe_output(&output)
}

fn parse_probe_output(output: &str) -> Result<ProbeResult> {
    let lines: Vec<&str> = output.lines().collect();
    let tool_path = lines.first().unwrap_or(&"none").trim();
    let fish_path = lines.get(1).unwrap_or(&"none").trim();

    let tool = if tool_path == "none" {
        SessionTool::None
    } else if tool_path.contains("shpool") {
        SessionTool::Shpool {
            path: tool_path.to_string(),
        }
    } else if tool_path.contains("abduco") {
        SessionTool::Abduco {
            path: tool_path.to_string(),
        }
    } else {
        SessionTool::None
    };

    let fish = if fish_path == "none" {
        None
    } else {
        Some(fish_path.to_string())
    };

    Ok(ProbeResult {
        tool,
        fish_path: fish,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_shpool_and_fish() {
        let output = "/home/user/.nix-profile/bin/shpool\n/home/user/.nix-profile/bin/fish\n";
        let result = parse_probe_output(output).unwrap();
        assert!(matches!(result.tool, SessionTool::Shpool { .. }));
        assert_eq!(result.tool.path(), "/home/user/.nix-profile/bin/shpool");
        assert_eq!(
            result.fish_path.as_deref(),
            Some("/home/user/.nix-profile/bin/fish")
        );
    }

    #[test]
    fn test_parse_none() {
        let output = "none\nnone\n";
        let result = parse_probe_output(output).unwrap();
        assert!(result.tool.is_none());
        assert!(result.fish_path.is_none());
    }

    #[test]
    fn test_parse_abduco_no_fish() {
        let output = "/usr/bin/abduco\nnone\n";
        let result = parse_probe_output(output).unwrap();
        assert!(matches!(result.tool, SessionTool::Abduco { .. }));
        assert!(result.fish_path.is_none());
    }
}
