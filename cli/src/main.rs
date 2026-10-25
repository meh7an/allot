use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand, ValueEnum};

mod classes;
mod daemon;
mod history;
mod hook;
mod install;
mod legacy;
mod probe;
mod registration;
mod sample;
mod status;
mod stop;
mod tray;

/// Inspect machine usage and leases, control the daemon, and run the Claude Code hook.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show machine usage, sessions, and their leases. Starts allotd when it is not running.
    Status,
    /// List active and queued leases.
    Leases {
        /// Show the most recent leases in any state instead.
        #[arg(long)]
        all: bool,
    },
    /// Stop an active lease's command and its process tree, whichever session holds it. The
    /// session is told the user stopped it.
    Kill {
        /// The lease ID, as `allot leases` prints it.
        id: i64,
    },
    /// Remove a queued lease before it starts, whichever session asked for it. The session
    /// does not run the command.
    Remove {
        /// The lease ID, as `allot leases` prints it.
        id: i64,
    },
    /// Print the heavy-command rules and class profiles in effect, as Markdown.
    Classes,
    /// Show past jobs and what they needed, or the runs of one job.
    History {
        /// A fingerprint as `allot history` prints it (quote it: it contains `|`), to list that
        /// job's runs.
        fingerprint: Option<String>,
        /// Most runs to list (default 20).
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Start, stop, or check the background daemon.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Record this process's ancestry, environment, and optionally stdin to a file, or print
    /// live machine samples with `--sample`.
    Probe(probe::ProbeArgs),
    /// Copy the built binaries into %USERPROFILE%\.allot\bin, restart allotd on them
    /// when it runs, install and start the tray app, copy the resource-guard skill, register
    /// the MCP server, hooks, and CLAUDE.md rule with Claude Code, register the logon task,
    /// and print what is left.
    Install {
        /// Directory holding the built allot.exe, allotd.exe, and allot-mcp.exe (default: this
        /// executable's directory, such as target\release).
        #[arg(long, value_name = "DIR")]
        from: Option<PathBuf>,
        /// The tray app's publish folder (default: the tray folder in the --from directory,
        /// as in a release).
        #[arg(long, value_name = "DIR")]
        tray: Option<PathBuf>,
        /// Turn the tray app's Launch at logon on or off (default: on the first time the tray
        /// app is installed, unchanged after that).
        #[arg(long, value_name = "STATE")]
        launch_at_logon: Option<Switch>,
        /// Leave Claude Code alone: no skill, MCP server, hooks, or CLAUDE.md rule.
        #[arg(long)]
        no_claude: bool,
        /// Do not register the scheduled task that starts allotd at logon.
        #[arg(long)]
        no_logon_task: bool,
    },
    /// Quit and remove the tray app, stop allotd, remove the binaries and the skill copy,
    /// unregister them from Claude Code, and print what is left. Keeps the database, token,
    /// config, settings, and logs.
    Uninstall {
        /// Also delete the database, token, config, and logs.
        #[arg(long)]
        purge: bool,
    },
    /// Claude Code hooks. They always exit 0 and report a deny in their JSON output.
    Hook {
        #[command(subcommand)]
        event: HookEvent,
    },
}

#[derive(Debug, Subcommand)]
enum HookEvent {
    /// PreToolUse for the Bash and PowerShell tools: reads the hook payload from stdin,
    /// denies heavy commands without a covering lease, and binds the lease to the call.
    PreTool,
    /// PostToolUse and PostToolUseFailure for the Bash and PowerShell tools: tells Claude,
    /// once, when the user stopped one of its jobs or removed a queued request.
    PostTool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Switch {
    On,
    Off,
}

#[derive(Debug, Subcommand)]
enum DaemonAction {
    /// Start allotd in the background unless it is already running.
    Start,
    /// Ask allotd to shut down and wait until it has.
    Stop,
    /// Report whether allotd is running, without starting it.
    Status,
}

fn main() -> ExitCode {
    let args = match Args::try_parse() {
        Ok(args) => args,
        // `--help` and `--version` exit 0 and print as usual.
        Err(error) if hook::invoked() && error.exit_code() != 0 => {
            return hook::argument_error(&error);
        }
        Err(error) => error.exit(),
    };
    let result = match args.command {
        Command::Status => status::status(),
        Command::Leases { all } => status::leases(all),
        Command::Kill { id } => stop::kill(id),
        Command::Remove { id } => stop::remove(id),
        Command::Classes => classes::classes(),
        Command::History { fingerprint, limit } => history::history(fingerprint.as_deref(), limit),
        Command::Daemon { action } => match action {
            DaemonAction::Start => daemon::start(),
            DaemonAction::Stop => daemon::stop(),
            DaemonAction::Status => daemon::status(),
        },
        Command::Install {
            from,
            tray,
            launch_at_logon,
            no_claude,
            no_logon_task,
        } => install::install(install::Options {
            from: from.as_deref(),
            tray: tray.as_deref(),
            launch_at_logon: launch_at_logon.map(|state| matches!(state, Switch::On)),
            setup: registration::Setup {
                claude: !no_claude,
                logon_task: !no_logon_task,
            },
        }),
        Command::Uninstall { purge } => install::uninstall(purge),
        Command::Probe(args) => probe::run(&args),
        Command::Hook { event } => match event {
            HookEvent::PreTool => return hook::pre_tool(),
            HookEvent::PostTool => return hook::post_tool(),
        },
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error:?}");
            ExitCode::FAILURE
        }
    }
}
