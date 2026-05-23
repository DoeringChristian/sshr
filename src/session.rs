use anyhow::{bail, Context, Result};
use owo_colors::OwoColorize;
use std::collections::HashSet;
use std::io::{self, Write};

use crate::probe::SessionTool;
use crate::ssh::SshContext;

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub name: String,
    pub raw_line: String,
}

pub fn list_sessions(
    ssh: &SshContext,
    host: &str,
    tool: &SessionTool,
    extra_args: &[String],
) -> Result<Vec<SessionEntry>> {
    let cmd = match tool {
        SessionTool::Shpool { path } => format!("{path} list 2>/dev/null"),
        SessionTool::Abduco { path } => format!("{path} 2>&1"),
        SessionTool::None => return Ok(vec![]),
    };
    let output = ssh.run_capture(host, extra_args, &cmd)?;
    Ok(parse_session_list(&output))
}

fn parse_session_list(output: &str) -> Vec<SessionEntry> {
    output
        .lines()
        .skip(1) // skip header
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let name = line.split_whitespace().next().unwrap_or("").to_string();
            SessionEntry {
                name,
                raw_line: line.to_string(),
            }
        })
        .collect()
}

pub fn new_session_name(
    ssh: &SshContext,
    host: &str,
    tool: &SessionTool,
    extra_args: &[String],
) -> Result<String> {
    let sessions = list_sessions(ssh, host, tool, extra_args)?;
    let existing: HashSet<&str> = sessions.iter().map(|s| s.name.as_str()).collect();
    let mut i = 0;
    loop {
        let name = format!("s{i}");
        if !existing.contains(name.as_str()) {
            return Ok(name);
        }
        i += 1;
    }
}

pub fn pick_session_interactive(
    ssh: &SshContext,
    host: &str,
    tool: &SessionTool,
    extra_args: &[String],
) -> Result<String> {
    let sessions = list_sessions(ssh, host, tool, extra_args)?;
    if sessions.is_empty() {
        bail!("no existing sessions on {}", host);
    }

    eprintln!("Sessions on {}:", host.cyan().bold());
    for (i, entry) in sessions.iter().enumerate() {
        eprintln!("  [{}] {}", (i + 1).to_string().bold(), entry.raw_line);
    }

    eprint!("Select session: ");
    io::stderr().flush()?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read input")?;
    let input = input.trim();

    let idx: usize = input.parse().context("invalid selection")?;
    if idx < 1 || idx > sessions.len() {
        bail!("selection out of range");
    }

    Ok(sessions[idx - 1].name.clone())
}

pub fn kill_sessions(
    ssh: &SshContext,
    host: &str,
    tool: &SessionTool,
    sessions: &[String],
) -> Result<()> {
    let path = tool.path();
    if path.is_empty() {
        bail!("no session tool found on remote");
    }
    let session_list = sessions.join(" ");
    let cmd = format!("{path} kill {session_list}");
    ssh.run_capture(host, &[], &cmd)?;
    Ok(())
}

pub fn clean_detached(ssh: &SshContext, host: &str, tool: &SessionTool) -> Result<()> {
    let sessions = list_sessions(ssh, host, tool, &[])?;
    let detached: Vec<&str> = sessions
        .iter()
        .filter(|s| s.raw_line.contains("detached"))
        .map(|s| s.name.as_str())
        .collect();

    if detached.is_empty() {
        eprintln!("No detached sessions.");
        return Ok(());
    }

    eprintln!(
        "Killing detached sessions: {}",
        detached.join(", ").green()
    );
    let names: Vec<String> = detached.iter().map(|s| s.to_string()).collect();
    kill_sessions(ssh, host, tool, &names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_session_list() {
        let output = "NAME    STARTED_AT      STATUS\n\
                       s0    2026-05-22T18:02:29.300+00:00   attached\n\
                       s1    2026-05-22T18:03:00.000+00:00   detached\n";
        let sessions = parse_session_list(output);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name, "s0");
        assert_eq!(sessions[1].name, "s1");
    }

    #[test]
    fn test_parse_empty_session_list() {
        let output = "NAME    STARTED_AT      STATUS\n";
        let sessions = parse_session_list(output);
        assert!(sessions.is_empty());
    }
}
