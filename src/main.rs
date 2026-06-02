mod cmd;
mod reconnect;
mod session;
mod ssh;
mod upload;
mod verbose;

use anyhow::{bail, Result};
use base64::Engine;
use clap::Parser;
use owo_colors::OwoColorize;

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

    /// Subcommand or host, followed by optional SSH args
    #[arg(required = true, trailing_var_arg = true)]
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
    let first = &cli.args[0];

    match first.as_str() {
        "list" | "ls" => {
            let host = cli.args.get(1).cloned().unwrap_or_default();
            if host.is_empty() {
                bail!("usage: sshr list <host>");
            }
            cmd_list(&host, cli.all)
        }
        "attach" => {
            let host = cli.args.get(1).cloned().unwrap_or_default();
            if host.is_empty() {
                bail!("usage: sshr attach <host>");
            }
            let ssh_args: Vec<String> = cli.args[2..].to_vec();
            cmd_connect(&host, &ssh_args, true, cli.remote_cwd, cli.shell, cli.force_upload)
        }
        "kill" => {
            let host = cli.args.get(1).cloned().unwrap_or_default();
            if host.is_empty() {
                bail!("usage: sshr kill <host> [session...]");
            }
            let sessions: Vec<String> = cli.args[2..].to_vec();
            cmd_kill(&host, &sessions, cli.all)
        }
        "clean" => {
            let host = cli.args.get(1).cloned().unwrap_or_default();
            if host.is_empty() {
                bail!("usage: sshr clean <host>");
            }
            cmd_clean(&host, cli.all)
        }
        _ => {
            let host = first.clone();
            let ssh_args: Vec<String> = cli.args[1..].to_vec();
            cmd_connect(&host, &ssh_args, false, cli.remote_cwd, cli.shell, cli.force_upload)
        }
    }
}

fn ensure_remote_shpool(ssh: &SshContext, host: &str, extra_args: &[String], force_upload: bool) -> Result<()> {
    upload::ensure_shpool(ssh, host, extra_args, force_upload)
}

fn cmd_list(host: &str, all: bool) -> Result<()> {
    let ssh = SshContext::new()?;
    ensure_remote_shpool(&ssh, host, &[], false)?;

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
    ensure_remote_shpool(&ssh, host, &[], false)?;

    let to_kill = if sessions.is_empty() {
        let entries = session::list_sessions(&ssh, host, &[])?;
        let prefix = session::local_prefix();
        let entries: Vec<_> = entries
            .into_iter()
            .filter(|s| all || s.name.starts_with(&format!("{prefix}-")))
            .collect();
        if entries.is_empty() {
            eprintln!("No sessions on {host}.");
            return Ok(());
        }
        eprintln!("Sessions on {}:", host.cyan().bold());
        for entry in &entries {
            eprintln!("  {}", entry.raw_line);
        }
        eprint!("Sessions to kill (space-separated): ");
        std::io::Write::flush(&mut std::io::stderr())?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            return Ok(());
        }
        input.split_whitespace().map(String::from).collect()
    } else {
        sessions.to_vec()
    };

    session::kill_sessions(&ssh, host, &to_kill)
}

fn cmd_clean(host: &str, all: bool) -> Result<()> {
    let ssh = SshContext::new()?;
    ensure_remote_shpool(&ssh, host, &[], false)?;
    session::clean_detached(&ssh, host, all)
}

fn cmd_connect(
    host: &str,
    ssh_args: &[String],
    attach: bool,
    remote_cwd: Option<String>,
    shell: Option<String>,
    force_upload: bool,
) -> Result<()> {
    let ssh = SshContext::new()?;

    set_user_var("sshr_host", host);

    ensure_remote_shpool(&ssh, host, ssh_args, force_upload)?;

    let session_name = if attach {
        session::pick_session_interactive(&ssh, host, ssh_args)?
    } else {
        session::new_session_name(&ssh, host, ssh_args)?
    };

    set_user_var("sshr_session", &session_name);

    let remote_cmd = cmd::build_shpool_cmd(
        &session_name,
        shell.as_deref(),
        remote_cwd.as_deref(),
    );

    eprintln!(
        "Connecting to {} (session: {})...",
        host.cyan().bold(),
        session_name.green().bold()
    );

    reconnect::run_with_reconnect(|| {
        ssh.run_interactive(host, ssh_args, Some(&remote_cmd))
    })
}
