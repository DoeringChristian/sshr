use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::path::PathBuf;

use crate::config::{EnvDirective, HostConfig};
use crate::ssh::SshContext;
use crate::vlog;

pub const REMOTE_SHPOOL_PATH: &str = "$HOME/.local/share/sshr/bin/shpool";
pub const REMOTE_SOCKET_PATH: &str = "$HOME/.local/run/sshr/shpool.socket";
pub const REMOTE_LAUNCH: &str = "$HOME/.local/share/sshr/init/launch.sh";
const REMOTE_SHPOOL_DIR: &str = "~/.local/share/sshr/bin";
const REMOTE_SOCKET_DIR: &str = "~/.local/run/sshr";
const REMOTE_INIT_DIR: &str = "$HOME/.local/share/sshr/init";

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

/// Upload shpool binary to the remote. Returns true if successful.
fn upload_shpool(
    ssh: &SshContext,
    host: &str,
    extra_args: &[String],
) -> Result<bool> {
    let platform = detect_remote_platform(ssh, host, extra_args)?;
    let binary_name = platform.binary_name();
    vlog!("remote platform: {}-{}", platform.os, platform.arch);

    let shpool_dir = match find_shpool_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!(
                "{}: no local shpool binaries found ({})",
                "warning".yellow().bold(),
                e
            );
            return Ok(false);
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
        return Ok(false);
    }

    vlog!("upload: local binary = {}", local_binary.display());
    vlog!("upload: remote path = {REMOTE_SHPOOL_DIR}/shpool");
    eprintln!("Uploading shpool to {}...", host.cyan().bold());

    ssh.run_capture(host, extra_args, &format!("mkdir -p {REMOTE_SHPOOL_DIR}"))?;

    ssh.scp_upload(
        host,
        extra_args,
        &local_binary,
        &format!("{REMOTE_SHPOOL_DIR}/shpool"),
    )?;

    ssh.run_capture(
        host,
        extra_args,
        &format!("chmod +x {REMOTE_SHPOOL_DIR}/shpool"),
    )?;

    eprintln!("{}", "Done.".dimmed());
    Ok(true)
}

/// Ensure sshr's own shpool is on the remote. Upload if missing.
pub fn ensure_shpool(
    ssh: &SshContext,
    host: &str,
    extra_args: &[String],
    force: bool,
    host_cfg: &HostConfig,
) -> Result<()> {
    if !force && has_sshr_shpool(ssh, host, extra_args)? {
        vlog!("shpool: present at {REMOTE_SHPOOL_PATH}");
    } else {
        if force {
            vlog!("shpool: forcing upload (--force-upload)");
        } else {
            vlog!("shpool: missing, uploading");
        }
        if !upload_shpool(ssh, host, extra_args)? {
            anyhow::bail!("failed to install shpool on remote");
        }
    }

    ssh.run_capture(host, extra_args, &format!("mkdir -p {REMOTE_SOCKET_DIR}"))?;

    ensure_init_files(ssh, host, extra_args, host_cfg)?;
    Ok(())
}

fn build_env_exports(env: &[EnvDirective]) -> String {
    let mut lines = Vec::new();
    for directive in env {
        let EnvDirective::Set(name, value) = directive;
        let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
        lines.push(format!("export {name}=\"{escaped}\""));
    }
    lines.join("\n")
}

fn ensure_init_files(
    ssh: &SshContext,
    host: &str,
    extra_args: &[String],
    host_cfg: &HostConfig,
) -> Result<()> {
    let env_block = build_env_exports(&host_cfg.env);
    let env_section = if env_block.is_empty() {
        String::new()
    } else {
        format!("{env_block}\n")
    };

    let script = format!(
        r#"mkdir -p {REMOTE_INIT_DIR}/zsh {REMOTE_INIT_DIR}/fish/vendor_conf.d
cat > {REMOTE_INIT_DIR}/launch.sh << 'SSHR_EOF'
#!/bin/sh
export SSH_CONNECTION="${{SSH_CONNECTION:-sshr}}"
{env_section}login_shell="${{1:-$SHELL}}"
if [ "${{login_shell#/}}" = "$login_shell" ]; then
    login_shell=$(command -v "$login_shell" 2>/dev/null || echo "$login_shell")
fi
shell_name=$(basename "$login_shell")
init_dir="$HOME/.local/share/sshr/init"

case "$shell_name" in
    bash) exec env ENV="$init_dir/bash_init.sh" "$login_shell" --posix ;;
    zsh)  exec env ZDOTDIR="$init_dir/zsh" "$login_shell" ;;
    fish) exec env XDG_DATA_DIRS="$init_dir:${{XDG_DATA_DIRS:-/usr/local/share:/usr/share}}" "$login_shell" ;;
    *)    exec "$login_shell" ;;
esac
SSHR_EOF
chmod +x {REMOTE_INIT_DIR}/launch.sh
cat > {REMOTE_INIT_DIR}/bash_init.sh << 'SSHR_EOF'
set +o posix
unset ENV
[ -f ~/.bashrc ] && . ~/.bashrc
__sshr_osc7() {{ printf '\033]7;file://%s%s\a' "$(hostname)" "$PWD"; }}
PROMPT_COMMAND="${{PROMPT_COMMAND:+$PROMPT_COMMAND; }}__sshr_osc7"
SSHR_EOF
cat > {REMOTE_INIT_DIR}/zsh/.zshenv << 'SSHR_EOF'
ZDOTDIR="$HOME"
[ -f "$ZDOTDIR/.zshenv" ] && . "$ZDOTDIR/.zshenv"
__sshr_osc7() {{ printf '\033]7;file://%s%s\a' "$(hostname)" "$PWD" }}
precmd_functions+=(__sshr_osc7)
SSHR_EOF
cat > {REMOTE_INIT_DIR}/fish/vendor_conf.d/sshr.fish << 'SSHR_EOF'
function __sshr_osc7 --on-event fish_prompt
    printf '\e]7;file://%s%s\a' (hostname) $PWD
end
SSHR_EOF
"#
    );
    ssh.run_capture(host, extra_args, &script)?;
    vlog!("init: created shell init files at {REMOTE_INIT_DIR}");
    Ok(())
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

    match arch.as_str() {
        "amd64" => arch = "x86_64".into(),
        "arm64" => arch = "aarch64".into(),
        _ => {}
    }

    Ok(RemotePlatform { os, arch })
}

fn find_shpool_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("SSHR_SHPOOL_DIR") {
        let path = PathBuf::from(dir);
        if path.is_dir() {
            return Ok(path);
        }
    }

    let exe = std::env::current_exe()?.canonicalize()?;

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
