use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::path::PathBuf;

use crate::probe::SessionTool;
use crate::ssh::SshContext;

const REMOTE_SHPOOL_PATH: &str = "$HOME/.local/share/sshr/bin/shpool";
const REMOTE_SHPOOL_DIR: &str = "~/.local/share/sshr/bin";

#[derive(Debug)]
struct RemotePlatform {
    os: String,
    arch: String,
}

impl RemotePlatform {
    fn binary_name(&self) -> String {
        format!("shpool-{}-{}", self.os, self.arch)
    }
}

/// Check if sshr's own shpool already exists on the remote.
pub fn has_sshr_shpool(ssh: &SshContext, host: &str, extra_args: &[String]) -> Result<bool> {
    let output = ssh.run_capture(
        host,
        extra_args,
        &format!("test -x {REMOTE_SHPOOL_DIR}/shpool && echo yes || echo no"),
    )?;
    Ok(output.trim() == "yes")
}

/// Return the SessionTool pointing to sshr's managed shpool on the remote.
pub fn sshr_shpool_tool() -> SessionTool {
    SessionTool::Shpool {
        path: REMOTE_SHPOOL_PATH.to_string(),
    }
}

/// Upload shpool binary to the remote.
/// Returns `Some(SessionTool::Shpool)` if successful, `None` if no local binary available.
pub fn upload_shpool(
    ssh: &SshContext,
    host: &str,
    extra_args: &[String],
) -> Result<Option<SessionTool>> {
    let platform = detect_remote_platform(ssh, host, extra_args)?;
    let binary_name = platform.binary_name();

    let shpool_dir = match find_shpool_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!(
                "{}: no local shpool binaries found ({})",
                "warning".yellow().bold(),
                e
            );
            return Ok(None);
        }
    };

    let local_binary = shpool_dir.join(&binary_name);
    if !local_binary.exists() {
        eprintln!(
            "{}: no shpool binary for {} (expected {})",
            "warning".yellow().bold(),
            binary_name.dimmed(),
            local_binary.display().to_string().dimmed(),
        );
        return Ok(None);
    }

    eprintln!("Uploading shpool to {}...", host.cyan().bold());

    // Create sshr directory on remote
    ssh.run_capture(host, extra_args, &format!("mkdir -p {REMOTE_SHPOOL_DIR}"))?;

    // Upload binary
    ssh.scp_upload(
        host,
        &local_binary,
        &format!("{REMOTE_SHPOOL_DIR}/shpool"),
    )?;

    // Make executable
    ssh.run_capture(
        host,
        extra_args,
        &format!("chmod +x {REMOTE_SHPOOL_DIR}/shpool"),
    )?;

    eprintln!("{}", "Done.".dimmed());

    Ok(Some(sshr_shpool_tool()))
}

/// Ensure sshr's own shpool is on the remote. Upload if missing.
/// Returns the sshr-managed shpool tool, or None if we can't provide one.
pub fn ensure_shpool(
    ssh: &SshContext,
    host: &str,
    extra_args: &[String],
    force: bool,
) -> Result<Option<SessionTool>> {
    if !force && has_sshr_shpool(ssh, host, extra_args)? {
        return Ok(Some(sshr_shpool_tool()));
    }
    upload_shpool(ssh, host, extra_args)
}

fn detect_remote_platform(
    ssh: &SshContext,
    host: &str,
    extra_args: &[String],
) -> Result<RemotePlatform> {
    let output = ssh
        .run_capture(host, extra_args, "uname -sm")
        .context("failed to detect remote platform")?;
    let parts: Vec<&str> = output.trim().split_whitespace().collect();
    let os = parts.first().unwrap_or(&"unknown").to_lowercase();
    let mut arch = parts.get(1).unwrap_or(&"unknown").to_string();

    // Normalize arch names
    match arch.as_str() {
        "amd64" => arch = "x86_64".into(),
        "arm64" => arch = "aarch64".into(),
        _ => {}
    }

    Ok(RemotePlatform { os, arch })
}

fn find_shpool_dir() -> Result<PathBuf> {
    // Check SSHR_SHPOOL_DIR env var first
    if let Ok(dir) = std::env::var("SSHR_SHPOOL_DIR") {
        let path = PathBuf::from(dir);
        if path.is_dir() {
            return Ok(path);
        }
    }

    let exe = std::env::current_exe()?.canonicalize()?;

    // Walk up from the executable, checking each ancestor for shpool binaries.
    // This handles:
    //   - Installed layout: $prefix/bin/sshr → $prefix/shpool/bin/
    //   - Nix layout: $prefix/bin/sshr → $prefix/share/sshr/shpool/bin/
    //   - Dev layout: target/debug/sshr → ./shpool/bin/ (project root)
    let mut dir = exe.parent();
    while let Some(d) = dir {
        let repo_path = d.join("shpool/bin");
        if repo_path.is_dir() {
            return Ok(repo_path);
        }
        let nix_path = d.join("share/sshr/shpool/bin");
        if nix_path.is_dir() {
            return Ok(nix_path);
        }
        dir = d.parent();
    }

    anyhow::bail!("no shpool binary directory found")
}
