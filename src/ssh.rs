use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};

use crate::vlog;

pub struct SshContext {
    control_dir: PathBuf,
    control_path: String,
    ssh_cmd: String,
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
            ssh_cmd: "ssh".into(),
        })
    }

    #[cfg(test)]
    pub fn with_mock(ssh_cmd: &str, control_dir: &str) -> Self {
        let control_dir = PathBuf::from(control_dir);
        let _ = fs::create_dir_all(&control_dir);
        let control_path = format!("{}/%r@%h:%p", control_dir.display());
        Self {
            control_dir,
            control_path,
            ssh_cmd: ssh_cmd.into(),
        }
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

    fn control_cmd(&self, op: &str, host: &str, extra_args: &[String]) -> Command {
        let mut cmd = Command::new(&self.ssh_cmd);
        cmd.args(["-O", op])
            .arg("-o")
            .arg(format!("ControlPath={}", self.control_path))
            .arg(host)
            .args(extra_args)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        cmd
    }

    /// Pre-flight check: if the control master is dead but its socket lingers,
    /// remove the stale socket so the next connection can create a fresh master.
    pub fn clean_stale_master(&self, host: &str, extra_args: &[String]) {
        let check = self.control_cmd("check", host, extra_args).status();
        if !matches!(check, Ok(s) if s.success()) {
            self.remove_stale_sockets(host);
        }
    }

    fn remove_stale_sockets(&self, host: &str) {
        let pattern = format!("{}/*@{}:*", self.control_dir.display(), host);
        if let Ok(paths) = glob::glob(&pattern) {
            for path in paths.flatten() {
                vlog!("ssh: removing stale socket {}", path.display());
                let _ = fs::remove_file(path);
            }
        }
    }

    /// Run SSH interactively with inherited stdio.
    pub fn run_interactive(
        &self,
        host: &str,
        extra_args: &[String],
        remote_cmd: Option<&str>,
    ) -> Result<ExitStatus> {
        let mut cmd = Command::new(&self.ssh_cmd);
        cmd.args(self.mux_args());
        cmd.arg(host);
        cmd.args(extra_args);
        if let Some(remote) = remote_cmd {
            cmd.arg("-t");
            cmd.arg(remote);
        }
        vlog!("exec: {cmd:?}");
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
        let mut cmd = Command::new(&self.ssh_cmd);
        cmd.args(self.mux_args())
            .arg(host)
            .args(extra_args)
            .arg(remote_cmd);
        vlog!("exec: {cmd:?}");
        let output = cmd
            .stderr(Stdio::null())
            .output()
            .context("failed to execute ssh")?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Tear down control master and clean up socket files.
    pub fn drop_control_master(&self, host: &str, extra_args: &[String]) {
        use std::time::{Duration, Instant};
        use std::thread;

        vlog!("ssh: tearing down control master");
        let child = self.control_cmd("exit", host, extra_args).spawn();

        if let Ok(mut child) = child {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() >= deadline => {
                        vlog!("ssh: -O exit timed out, killing");
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                    Ok(None) => thread::sleep(Duration::from_millis(50)),
                    Err(_) => break,
                }
            }
        }
        self.remove_stale_sockets(host);
    }

    /// Upload a file via SCP using the same control socket.
    pub fn scp_upload(
        &self,
        host: &str,
        extra_args: &[String],
        local: &std::path::Path,
        remote: &str,
    ) -> Result<()> {
        let mut cmd = Command::new("scp");
        cmd.arg("-o")
            .arg(format!("ControlPath={}", self.control_path))
            .args(translate_for_scp(extra_args))
            .arg(local)
            .arg(format!("{host}:{remote}"));
        vlog!("exec: {cmd:?}");
        let status = cmd
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("failed to execute scp")?;
        anyhow::ensure!(status.success(), "scp upload failed");
        Ok(())
    }
}

/// Translate ssh-style args to scp-style. scp uses `-P` (capital) for the
/// port; lowercase `-p` would mean "preserve times" instead.
fn translate_for_scp(args: &[String]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut i = 0;
    while i < args.len() {
        if args[i] == "-p" {
            out.push("-P".into());
        } else {
            out.push(args[i].clone());
        }
        i += 1;
    }
    out
}

fn dirs() -> PathBuf {
    home_dir().join(".ssh")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_ID: AtomicUsize = AtomicUsize::new(0);

    struct MockSsh {
        dir: PathBuf,
        script_path: PathBuf,
    }

    impl MockSsh {
        fn new(behavior: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "sshr-test-{}-{}",
                std::process::id(),
                TEST_ID.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir_all(&dir).unwrap();

            let script_path = dir.join("mock_ssh");
            let log_path = dir.join("calls.log");

            let script = format!(
                "#!/bin/sh\necho \"$@\" >> \"{}\"\n{}",
                log_path.display(),
                behavior,
            );
            fs::write(&script_path, script).unwrap();

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();
            }

            Self { dir, script_path }
        }

        fn path(&self) -> &str {
            self.script_path.to_str().unwrap()
        }

        fn calls(&self) -> Vec<String> {
            let log = self.dir.join("calls.log");
            if log.exists() {
                fs::read_to_string(&log)
                    .unwrap()
                    .lines()
                    .map(String::from)
                    .collect()
            } else {
                vec![]
            }
        }
    }

    impl Drop for MockSsh {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    fn make_ctx(mock: &MockSsh) -> SshContext {
        let ctl_dir = mock.dir.join("ctl");
        SshContext::with_mock(mock.path(), ctl_dir.to_str().unwrap())
    }

    fn counting_script(mock: &MockSsh, fail_count: u32) {
        let script = format!(
            "#!/bin/sh\n\
             MOCK_DIR=\"{dir}\"\n\
             echo \"$@\" >> \"{dir}/calls.log\"\n\
             for arg in \"$@\"; do\n\
               if [ \"$arg\" = \"exit\" ] || [ \"$arg\" = \"check\" ]; then exit 0; fi\n\
             done\n\
             COUNTER=$(cat \"$MOCK_DIR/counter\" 2>/dev/null || echo 0)\n\
             COUNTER=$((COUNTER + 1))\n\
             echo $COUNTER > \"$MOCK_DIR/counter\"\n\
             if [ \"$COUNTER\" -le {fail_count} ]; then exit 255; fi\n\
             exit 0",
            dir = mock.dir.display()
        );
        fs::write(&mock.script_path, script).unwrap();
    }

    // ---- drop_control_master ----

    #[test]
    fn drop_control_master_sends_exit_command() {
        let mock = MockSsh::new("exit 0");
        let ctx = make_ctx(&mock);

        ctx.drop_control_master("fermat", &[]);

        let calls = mock.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].contains("-O exit"), "should send -O exit, got: {}", calls[0]);
        assert!(calls[0].contains("fermat"), "should target correct host");
        assert!(calls[0].contains("ControlPath="));
    }

    #[test]
    fn drop_control_master_passes_extra_args() {
        let mock = MockSsh::new("exit 0");
        let ctx = make_ctx(&mock);
        let args = vec!["-p".into(), "2222".into()];

        ctx.drop_control_master("server", &args);

        let calls = mock.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].contains("-p 2222"), "extra args missing: {}", calls[0]);
        assert!(calls[0].contains("server"));
    }

    #[test]
    fn drop_control_master_tolerates_ssh_failure() {
        let mock = MockSsh::new("exit 1");
        let ctx = make_ctx(&mock);

        ctx.drop_control_master("host", &[]);
        assert_eq!(mock.calls().len(), 1);
    }

    // ---- drop_control_master cleans stale sockets ----

    #[test]
    fn drop_control_master_removes_stale_socket_files() {
        let mock = MockSsh::new("exit 0");
        let ctx = make_ctx(&mock);

        // Create a fake stale socket file
        let stale = ctx.control_dir.join("user@fermat:22");
        fs::write(&stale, "").unwrap();
        assert!(stale.exists());

        ctx.drop_control_master("fermat", &[]);

        assert!(!stale.exists(), "stale socket should be removed");
    }

    // ---- run_interactive ----

    #[test]
    fn run_interactive_returns_exit_code() {
        let mock = MockSsh::new("exit 42");
        let ctx = make_ctx(&mock);

        let status = ctx.run_interactive("host", &[], None).unwrap();
        assert_eq!(status.code(), Some(42));
    }

    #[test]
    fn run_interactive_passes_remote_command() {
        let mock = MockSsh::new("exit 0");
        let ctx = make_ctx(&mock);

        ctx.run_interactive("host", &[], Some("shpool attach foo")).unwrap();

        let calls = mock.calls();
        assert!(calls[0].contains("-t"), "should add -t for remote command");
        assert!(calls[0].contains("shpool attach foo"));
    }

    #[test]
    fn run_interactive_exit_255_is_ssh_error() {
        let mock = MockSsh::new("exit 255");
        let ctx = make_ctx(&mock);

        let status = ctx.run_interactive("host", &[], None).unwrap();
        assert_eq!(status.code(), Some(255));
        assert!(!status.success());
    }

    // ---- Full reconnect scenario with mock SSH ----

    #[test]
    fn full_scenario_stale_master_then_recovery() {
        let mock = MockSsh::new("");
        counting_script(&mock, 1);

        let ctx = make_ctx(&mock);
        let master_killed = std::sync::atomic::AtomicBool::new(false);

        let result = crate::reconnect::run_reconnect_loop(
            || ctx.run_interactive("fermat", &[], Some("shpool attach s1")),
            || {
                ctx.drop_control_master("fermat", &[]);
                master_killed.store(true, std::sync::atomic::Ordering::SeqCst);
            },
            || panic!("immediate retry should succeed without prompting"),
            || false,
        );

        assert!(result.is_ok());
        assert!(master_killed.load(std::sync::atomic::Ordering::SeqCst));
        assert!(mock.calls().iter().any(|c| c.contains("-O exit")), "master should be killed");
    }

    #[test]
    fn full_scenario_healthy_master_not_killed() {
        let mock = MockSsh::new("exit 0");
        let ctx = make_ctx(&mock);
        let master_killed = std::sync::atomic::AtomicBool::new(false);

        let result = crate::reconnect::run_reconnect_loop(
            || ctx.run_interactive("fermat", &[], Some("shpool attach s1")),
            || { master_killed.store(true, std::sync::atomic::Ordering::SeqCst); },
            || panic!("should not wait"),
            || false,
        );

        assert!(result.is_ok());
        assert!(!master_killed.load(std::sync::atomic::Ordering::SeqCst),
            "must not kill a healthy master from another session");
    }

    #[test]
    fn full_scenario_network_down_then_recovery() {
        let mock = MockSsh::new("");
        counting_script(&mock, 4);

        let ctx = make_ctx(&mock);
        let error_count = std::sync::atomic::AtomicUsize::new(0);
        let wait_count = std::sync::atomic::AtomicUsize::new(0);

        let result = crate::reconnect::run_reconnect_loop(
            || ctx.run_interactive("fermat", &[], None),
            || {
                ctx.drop_control_master("fermat", &[]);
                error_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            },
            || {
                wait_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                true
            },
            || false,
        );

        assert!(result.is_ok());
        assert!(error_count.load(std::sync::atomic::Ordering::SeqCst) >= 2,
            "should kill master on each 255 cycle");
        assert!(wait_count.load(std::sync::atomic::Ordering::SeqCst) >= 1,
            "should prompt user when retry also fails");
    }
}
