mod cmd;
mod kitty;
mod probe;
mod reconnect;
mod session;
mod ssh;
mod upload;

use anyhow::{bail, Result};
use clap::Parser;
use owo_colors::OwoColorize;

use probe::SessionTool;
use ssh::SshContext;

#[derive(Parser)]
#[command(
    name = "sshr",
    version,
    about = "Resilient SSH sessions with automatic reconnection"
)]
struct Cli {
    /// Attach to an existing session
    #[arg(short = 'a', long)]
    attach: bool,

    /// Start in the given remote directory
    #[arg(long)]
    remote_cwd: Option<String>,

    /// Shell to use on remote (default: auto-detect fish)
    #[arg(long)]
    shell: Option<String>,

    /// Force upload of shpool binary even if already installed on remote
    #[arg(long)]
    force_upload: bool,

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
    let first = &cli.args[0];

    match first.as_str() {
        "list" | "ls" => {
            let host = cli.args.get(1).cloned().unwrap_or_default();
            if host.is_empty() {
                bail!("usage: sshr list <host>");
            }
            cmd_list(&host)
        }
        "kill" => {
            let host = cli.args.get(1).cloned().unwrap_or_default();
            if host.is_empty() {
                bail!("usage: sshr kill <host> [session...]");
            }
            let sessions: Vec<String> = cli.args[2..].to_vec();
            cmd_kill(&host, &sessions)
        }
        "clean" => {
            let host = cli.args.get(1).cloned().unwrap_or_default();
            if host.is_empty() {
                bail!("usage: sshr clean <host>");
            }
            cmd_clean(&host)
        }
        _ => {
            let host = first.clone();
            let ssh_args: Vec<String> = cli.args[1..].to_vec();
            cmd_connect(&host, &ssh_args, cli.attach, cli.remote_cwd, cli.shell, cli.force_upload)
        }
    }
}

fn cmd_list(host: &str) -> Result<()> {
    let ssh = SshContext::new()?;
    let probe_result = probe::probe_remote(&ssh, host, &[])?;
    if probe_result.tool.is_none() {
        bail!("no session tool found on {host}");
    }
    // Print full output including header
    let cmd = match &probe_result.tool {
        SessionTool::Shpool { path } => format!("{path} list 2>/dev/null"),
        SessionTool::Abduco { path } => format!("{path} 2>&1"),
        SessionTool::None => unreachable!(),
    };
    let output = ssh.run_capture(host, &[], &cmd)?;
    print!("{output}");
    Ok(())
}

fn cmd_kill(host: &str, sessions: &[String]) -> Result<()> {
    let ssh = SshContext::new()?;
    let probe_result = probe::probe_remote(&ssh, host, &[])?;
    if probe_result.tool.is_none() {
        bail!("no session tool found on {host}");
    }

    let to_kill = if sessions.is_empty() {
        // Interactive: list and ask
        let entries = session::list_sessions(&ssh, host, &probe_result.tool, &[])?;
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

    session::kill_sessions(&ssh, host, &probe_result.tool, &to_kill)
}

fn cmd_clean(host: &str) -> Result<()> {
    let ssh = SshContext::new()?;
    let probe_result = probe::probe_remote(&ssh, host, &[])?;
    if probe_result.tool.is_none() {
        bail!("no session tool found on {host}");
    }
    session::clean_detached(&ssh, host, &probe_result.tool)
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

    // Kitty: set host
    if kitty::is_kitty() {
        kitty::set_user_var("sshr_host", host);
    }

    // Probe remote for shell
    let probe_result = probe::probe_remote(&ssh, host, ssh_args)?;

    // Resolve shell: explicit > fish > default
    let shell_path = shell.or(probe_result.fish_path);

    // Use probe result if available, otherwise upload shpool as fallback
    let tool = if !probe_result.tool.is_none() && !force_upload {
        probe_result.tool
    } else {
        upload::ensure_shpool(&ssh, host, ssh_args, force_upload)?
            .unwrap_or(probe_result.tool)
    };

    // Determine session name
    let session_name = if tool.is_none() {
        None
    } else if attach {
        Some(session::pick_session_interactive(
            &ssh, host, &tool, ssh_args,
        )?)
    } else {
        Some(session::new_session_name(&ssh, host, &tool, ssh_args)?)
    };

    // Kitty: set session + tool
    if kitty::is_kitty() {
        if let Some(ref s) = session_name {
            kitty::set_user_var("sshr_session", s);
            kitty::set_user_var("sshr_tool", tool.path());
        }
    }

    // Build remote command
    let remote_cmd = session_name.as_ref().and_then(|s| {
        cmd::build_remote_cmd(&tool, s, shell_path.as_deref(), remote_cwd.as_deref())
    });

    // If no session tool and no shell, remote_cmd might still be None from cmd module
    // but we may have a shell-only or cwd-only command
    let remote_cmd = remote_cmd.or_else(|| {
        if tool.is_none() {
            cmd::build_remote_cmd(&tool, "", shell_path.as_deref(), remote_cwd.as_deref())
        } else {
            None
        }
    });

    // Status message
    if let Some(ref s) = session_name {
        eprintln!(
            "Connecting to {} (using {}, session: {})...",
            host.cyan().bold(),
            tool.name().green(),
            s.green().bold()
        );
    } else {
        eprintln!("Connecting to {}...", host.cyan().bold());
    }

    // Connect with reconnection
    reconnect::run_with_reconnect(|| {
        ssh.run_interactive(host, ssh_args, remote_cmd.as_deref())
    })
}
