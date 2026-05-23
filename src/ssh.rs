use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

pub struct SshContext {
    control_dir: PathBuf,
    control_path: String,
}

impl SshContext {
    pub fn new() -> Result<Self> {
        let control_dir = dirs().join("sshr-sockets");
        fs::create_dir_all(&control_dir)
            .context("failed to create SSH control socket directory")?;
        let control_path = format!("{}/%r@%h:%p", control_dir.display());
        Ok(Self {
            control_dir,
            control_path,
        })
    }

    fn mux_args(&self) -> Vec<String> {
        vec![
            "-o".into(),
            "ControlMaster=auto".into(),
            "-o".into(),
            format!("ControlPath={}", self.control_path),
            "-o".into(),
            "ControlPersist=10m".into(),
        ]
    }

    /// Run SSH interactively with inherited stdio.
    pub fn run_interactive(
        &self,
        host: &str,
        extra_args: &[String],
        remote_cmd: Option<&str>,
    ) -> Result<ExitStatus> {
        let mut cmd = Command::new("ssh");
        cmd.args(self.mux_args());
        cmd.arg(host);
        cmd.args(extra_args);
        if let Some(remote) = remote_cmd {
            cmd.arg("-t");
            cmd.arg(remote);
        }
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        cmd.status().context("failed to execute ssh")
    }

    /// Run SSH and capture stdout, suppressing stderr.
    pub fn run_capture(
        &self,
        host: &str,
        extra_args: &[String],
        remote_cmd: &str,
    ) -> Result<String> {
        let output = Command::new("ssh")
            .args(self.mux_args())
            .arg(host)
            .args(extra_args)
            .arg(remote_cmd)
            .stderr(Stdio::null())
            .output()
            .context("failed to execute ssh")?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Upload a file via SCP using the same control socket.
    pub fn scp_upload(&self, host: &str, local: &std::path::Path, remote: &str) -> Result<()> {
        let status = Command::new("scp")
            .arg("-o")
            .arg(format!("ControlPath={}", self.control_path))
            .arg(local)
            .arg(format!("{host}:{remote}"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("failed to execute scp")?;
        anyhow::ensure!(status.success(), "scp upload failed");
        Ok(())
    }
}

fn dirs() -> PathBuf {
    home_dir().join(".ssh")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}
