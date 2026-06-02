use anyhow::{bail, Context, Result};
use owo_colors::OwoColorize;
use std::collections::HashSet;
use std::io::{self, Write};

use crate::ssh::SshContext;
use crate::upload::{REMOTE_SHPOOL_PATH, REMOTE_SOCKET_PATH};
use crate::vlog;

pub fn local_prefix() -> String {
    let full = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".into());
    let short = full.split('.').next().unwrap_or(&full);
    short.to_lowercase().replace(' ', "-")
}

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub name: String,
    pub raw_line: String,
}

pub fn list_sessions(
    ssh: &SshContext,
    host: &str,
    extra_args: &[String],
) -> Result<Vec<SessionEntry>> {
    let cmd = format!(
        "{REMOTE_SHPOOL_PATH} --socket {REMOTE_SOCKET_PATH} list 2>/dev/null"
    );
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
    extra_args: &[String],
) -> Result<String> {
    let prefix = local_prefix();
    let sessions = list_sessions(ssh, host, extra_args)?;
    let existing: HashSet<&str> = sessions.iter().map(|s| s.name.as_str()).collect();
    let mut i = 0;
    loop {
        let name = format!("{prefix}-s{i}");
        if !existing.contains(name.as_str()) {
            vlog!("session: new = {name}");
            return Ok(name);
        }
        i += 1;
    }
}

pub fn pick_session_interactive(
    ssh: &SshContext,
    host: &str,
    extra_args: &[String],
) -> Result<String> {
    let prefix = local_prefix();
    let sessions: Vec<_> = list_sessions(ssh, host, extra_args)?
        .into_iter()
        .filter(|s| s.name.starts_with(&format!("{prefix}-")))
        .collect();
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

    let name = sessions[idx - 1].name.clone();
    vlog!("session: selected = {name}");
    Ok(name)
}

pub fn kill_sessions(
    ssh: &SshContext,
    host: &str,
    sessions: &[String],
) -> Result<()> {
    let session_list = sessions.join(" ");
    let cmd = format!(
        "{REMOTE_SHPOOL_PATH} --socket {REMOTE_SOCKET_PATH} kill {session_list}"
    );
    ssh.run_capture(host, &[], &cmd)?;
    Ok(())
}

pub fn clean_detached(ssh: &SshContext, host: &str, all: bool) -> Result<()> {
    let sessions = list_sessions(ssh, host, &[])?;
    let prefix = local_prefix();
    let detached: Vec<&str> = sessions
        .iter()
        .filter(|s| s.raw_line.contains("detached"))
        .filter(|s| all || s.name.starts_with(&format!("{prefix}-")))
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
    kill_sessions(ssh, host, &names)
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
