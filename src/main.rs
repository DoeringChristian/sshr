mod cmd;
mod config;
mod copy;
mod reconnect;
mod session;
mod signal;
mod ssh;
mod upload;
mod verbose;
mod wal;

use anyhow::Result;
use base64::Engine;
use clap::Parser;
use owo_colors::OwoColorize;

use config::HostConfig;
use ssh::SshContext;

fn set_user_var(key: &str, value: &str) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(value);
    eprint!("\x1b]1337;SetUserVar={key}={encoded}\x07");
}

#[derive(Parser)]
#[command(
    name = "sshr",
    version,
    about = "Resilient SSH sessions with automatic reconnection"
)]
struct Cli {
    /// Show/operate on sessions from all clients (not just this host)
    #[arg(short = 'a', long)]
    all: bool,

    /// Start in the given remote directory
    #[arg(long)]
    remote_cwd: Option<String>,

    /// Shell to use on remote (default: login shell)
    #[arg(long)]
    shell: Option<String>,

    /// Force upload of shpool binary even if already installed on remote
    #[arg(long)]
    force_upload: bool,

    /// Verbose: log paths and SSH commands
    #[arg(short = 'v', long)]
    verbose: bool,

    /// Remote host
    #[arg(required = true)]
    host: String,

    /// Subcommand (list, attach, kill, clean) or extra SSH args
    #[arg(trailing_var_arg = true)]
    args: Vec<String>,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}: {:#}", "error".red().bold(), err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    verbose::set(cli.verbose);
    let cfg = config::Config::load();
    let host = &cli.host;
    let subcmd = cli.args.first().map(|s| s.as_str());

    match subcmd {
        Some("list" | "ls") => cmd_list(host, cli.all),
        Some("attach") => {
            let ssh_args: Vec<String> = cli.args[1..].to_vec();
            let host_cfg = cfg.for_host(host);
            cmd_connect(host, &ssh_args, true, &cli, host_cfg)
        }
        Some("kill") => {
            let sessions: Vec<String> = cli.args[1..].to_vec();
            cmd_kill(host, &sessions, cli.all)
        }
        Some("clean") => cmd_clean(host, cli.all),
        _ => {
            let ssh_args = cli.args.clone();
            let host_cfg = cfg.for_host(host);
            cmd_connect(host, &ssh_args, false, &cli, host_cfg)
        }
    }
}

fn ensure_remote_shpool(
    ssh: &SshContext,
    host: &str,
    extra_args: &[String],
    force_upload: bool,
    host_cfg: &HostConfig,
) -> Result<()> {
    upload::ensure_shpool(ssh, host, extra_args, force_upload, host_cfg)
}

fn cmd_list(host: &str, all: bool) -> Result<()> {
    let ssh = SshContext::new()?;
    let sessions = session::list_sessions(&ssh, host, &[])?;
    let prefix = session::local_prefix();
    let filtered: Vec<&session::SessionEntry> = sessions
        .iter()
        .filter(|s| all || s.name.starts_with(&format!("{prefix}-")))
        .collect();

    if filtered.is_empty() {
        eprintln!("No sessions.");
    } else {
        for entry in filtered {
            println!("{}", entry.raw_line);
        }
    }
    Ok(())
}

fn cmd_kill(host: &str, sessions: &[String], all: bool) -> Result<()> {
    let ssh = SshContext::new()?;
    let to_kill = if sessions.is_empty() {
        session::pick_sessions_to_kill(&ssh, host, all)?
    } else {
        sessions.to_vec()
    };

    session::kill_sessions(&ssh, host, &to_kill)
}

fn cmd_clean(host: &str, all: bool) -> Result<()> {
    let ssh = SshContext::new()?;
    session::clean_detached(&ssh, host, all)
}

fn cmd_connect(
    host: &str,
    ssh_args: &[String],
    attach: bool,
    cli: &Cli,
    host_cfg: HostConfig,
) -> Result<()> {
    // Delegate to another command if configured
    if let Some(ref delegate) = host_cfg.delegate {
        let mut cmd = std::process::Command::new(delegate);
        cmd.arg(host);
        cmd.args(ssh_args);
        let status = cmd.status()?;
        std::process::exit(status.code().unwrap_or(1));
    }

    let ssh = SshContext::new()?;

    set_user_var("sshr_host", host);

    ensure_remote_shpool(&ssh, host, ssh_args, cli.force_upload, &host_cfg)?;

    wal::replay(&ssh, host);

    if !host_cfg.copy.is_empty() {
        copy::run_copy_directives(&ssh, host, ssh_args, &host_cfg.copy)?;
    }

    let session_name = if attach {
        session::pick_session_interactive(&ssh, host, ssh_args, cli.all)?
    } else {
        session::new_session_name(&ssh, host, ssh_args)?
    };

    set_user_var("sshr_session", &session_name);

    let shell = cli.shell.clone().or(host_cfg.shell.clone());
    let remote_cwd = cli.remote_cwd.clone().or(host_cfg.cwd.clone());

    let remote_cmd = cmd::build_shpool_cmd(
        &session_name,
        shell.as_deref(),
        remote_cwd.as_deref(),
    );

    signal::install_handlers();

    eprintln!(
        "Connecting to {} (session: {})...",
        host.cyan().bold(),
        session_name.green().bold()
    );

    let result = reconnect::run_with_reconnect(|| {
        ssh.run_interactive(host, ssh_args, Some(&remote_cmd))
    });

    wal::record_close(&ssh, host, &session_name);

    set_user_var("sshr_host", "");
    set_user_var("sshr_session", "");

    result
}
