use std::{env, path::PathBuf, process::ExitCode, time::Duration};

use allot_client::ClientError;
use clap::{Parser, Subcommand};
use clap_complete::Shell;

use crate::{
    output::{BAD, ColorWhen, paint},
    status::StatusMode,
};

mod classes;
mod completions;
mod daemon;
#[cfg(any(target_os = "linux", test))]
#[cfg_attr(
    not(target_os = "linux"),
    allow(dead_code, reason = "tested on every OS, used on Linux")
)]
mod desktop;
mod doctor;
mod history;
mod hook;
mod install;
#[cfg(windows)]
mod legacy;
mod output;
mod probe;
mod registered;
mod registration;
mod sample;
mod status;
mod stop;
#[cfg(windows)]
mod tray;
#[cfg(any(unix, test))]
#[cfg_attr(
    not(unix),
    allow(dead_code, reason = "tested on every OS, used on Linux and macOS")
)]
mod xdg;

const EXAMPLES: &str = "\
Examples:
  allot status                     machine, sessions, and their leases
  allot status --watch             the same, redrawn every 2 s
  allot leases --all               the most recent leases in any state
  allot kill 12                    stop lease 12's command and its processes
  allot history --json             past jobs as JSON
  allot doctor                     check the install and say what to fix
  allot completions fish > ~/.config/fish/completions/allot.fish

Run `allot <command> --help` for a command's options.";

/// Inspect machine usage and leases, control the daemon, and run the Claude Code hook.
#[derive(Debug, Parser)]
#[command(name = "allot", version, after_help = EXAMPLES)]
pub(crate) struct Args {
    /// When to colour the output: auto (on a terminal, unless NO_COLOR is set), always, or
    /// never.
    #[arg(long, global = true, value_name = "WHEN", default_value = "auto")]
    color: ColorWhen,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show machine usage, sessions, and their leases. Starts allotd when it is not running.
    #[command(after_help = "\
Examples:
  allot status
  allot status --watch --interval 5
  allot status --json | jq '.queue[].label'")]
    Status {
        /// Print the daemon's whole UI state as JSON (the `ui-state` schema in contract/).
        #[arg(long, conflicts_with = "watch")]
        json: bool,
        /// Redraw every interval until Ctrl-C.
        #[arg(long)]
        watch: bool,
        /// Seconds between redraws with --watch.
        #[arg(
            long,
            value_name = "SECS",
            default_value_t = 2,
            requires = "watch",
            value_parser = clap::value_parser!(u64).range(1..=3_600)
        )]
        interval: u64,
    },
    /// List active and queued leases.
    #[command(after_help = "\
Examples:
  allot leases
  allot leases --all --json")]
    Leases {
        /// Show the most recent leases in any state instead.
        #[arg(long)]
        all: bool,
        /// Print the leases as JSON, as allotd serves them.
        #[arg(long)]
        json: bool,
    },
    /// Stop an active lease's command and its process tree, whichever session holds it. The
    /// session is told the user stopped it.
    #[command(after_help = "\
Examples:
  allot leases        find the ID
  allot kill 12")]
    Kill {
        /// The lease ID, as `allot leases` prints it.
        id: i64,
        /// Print allotd's response as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Remove a queued lease before it starts, whichever session asked for it. The session
    /// does not run the command.
    #[command(after_help = "\
Examples:
  allot leases        find the ID
  allot remove 13")]
    Remove {
        /// The lease ID, as `allot leases` prints it.
        id: i64,
        /// Print allotd's response as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print the heavy-command rules and class profiles in effect, as Markdown.
    Classes,
    /// Show past jobs and what they needed, or the runs of one job.
    #[command(after_help = "\
Examples:
  allot history
  allot history 'cargo build|/home/me/app' --limit 5
  allot history --json")]
    History {
        /// A fingerprint as `allot history` prints it (quote it: it contains `|`), to list that
        /// job's runs.
        fingerprint: Option<String>,
        /// Most runs to list (default 20).
        #[arg(long)]
        limit: Option<u32>,
        /// Print the jobs, or the runs, as JSON, as allotd serves them.
        #[arg(long)]
        json: bool,
    },
    /// Start, stop, or check the background daemon.
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Check the binaries, PATH, config, allotd, and what `allot install` registers, and say
    /// what to fix. Changes nothing; exits 1 when a check fails.
    Doctor {
        /// Print the checks as JSON: name, outcome (ok, warn, fail), detail, fix.
        #[arg(long)]
        json: bool,
    },
    /// Print the completion script for a shell. `allot install` sets up bash and fish.
    #[command(after_help = "\
Examples:
  allot completions bash > ~/.local/share/bash-completion/completions/allot
  allot completions zsh > ~/.zfunc/_allot        then fpath+=~/.zfunc before compinit
  allot completions fish > ~/.config/fish/completions/allot.fish
  allot completions powershell | Out-String | Invoke-Expression   in $PROFILE")]
    Completions {
        /// The shell to complete in.
        shell: Shell,
    },
    /// Print the man page (roff), as packages install it as allot.1.
    #[command(after_help = "\
Examples:
  allot man | man -l -")]
    Man,
    /// Record this process's ancestry, environment, and optionally stdin to a file, or print
    /// live machine samples with `--sample`.
    Probe(probe::ProbeArgs),
    /// Copy the built binaries into ~/.allot/bin and link allot into ~/.local/bin, restart
    /// allotd on them when it runs, copy the resource-guard skill, install the allot icon
    /// (Linux), register the MCP server, hooks, and CLAUDE.md rule with Claude Code, set up the
    /// systemd user unit (Linux) or LaunchAgent (macOS) that starts allotd at login, and print
    /// what is left.
    #[cfg(not(windows))]
    Install {
        /// Directory holding the built allot, allotd, and allot-mcp (default: this
        /// executable's directory, such as target/release).
        #[arg(long, value_name = "DIR")]
        from: Option<PathBuf>,
        /// Leave Claude Code alone: no skill, MCP server, hooks, or CLAUDE.md rule.
        #[arg(long)]
        no_claude: bool,
        /// Do not set up the systemd user unit (Linux) or LaunchAgent (macOS) that starts
        /// allotd at login.
        #[arg(long)]
        no_autostart: bool,
    },
    /// Copy the built binaries into %USERPROFILE%\.allot\bin, restart allotd on them
    /// when it runs, install and start the tray app, copy the resource-guard skill, register
    /// the MCP server, hooks, and CLAUDE.md rule with Claude Code, register the logon task,
    /// and print what is left.
    #[cfg(windows)]
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
        #[arg(long, alias = "no-autostart")]
        no_logon_task: bool,
    },
    /// Stop allotd and remove its systemd user unit or LaunchAgent, the binaries, the link in
    /// ~/.local/bin, the skill copy, and on Linux the allot icon and the KDE widget's
    /// notifications file, unregister them from Claude Code, and print what is left. Keeps the
    /// database, token, config, and logs. The panel packages are removed with their own tools.
    #[cfg(not(windows))]
    Uninstall {
        /// Also delete the database, token, config, and logs.
        #[arg(long)]
        purge: bool,
    },
    /// Quit and remove the tray app, stop allotd, remove the binaries and the skill copy,
    /// unregister them from Claude Code, and print what is left. Keeps the database, token,
    /// config, settings, and logs.
    #[cfg(windows)]
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

#[cfg(windows)]
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
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
    Status {
        /// Print allotd's health as JSON, or {"running": false} when it is not running.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    // Before parsing, so that clap's help and errors follow --color too.
    if let Some(color) = output::color_in_args(env::args_os()) {
        color.apply();
    }
    let args = match Args::try_parse() {
        Ok(args) => args,
        // `--help` and `--version` exit 0 and print as usual.
        Err(error) if hook::invoked() && error.exit_code() != 0 => {
            return hook::argument_error(&error);
        }
        Err(error) => error.exit(),
    };
    args.color.apply();
    let result = match args.command {
        Command::Status {
            json,
            watch,
            interval,
        } => status::status(if watch {
            StatusMode::Watch(Duration::from_secs(interval))
        } else if json {
            StatusMode::Json
        } else {
            StatusMode::Text
        }),
        Command::Leases { all, json } => status::leases(all, json),
        Command::Kill { id, json } => stop::kill(id, json),
        Command::Remove { id, json } => stop::remove(id, json),
        Command::Classes => classes::classes(),
        Command::History {
            fingerprint,
            limit,
            json,
        } => history::history(fingerprint.as_deref(), limit, json),
        Command::Daemon { action } => match action {
            DaemonAction::Start => daemon::start(),
            DaemonAction::Stop => daemon::stop(),
            DaemonAction::Status { json } => daemon::status(json),
        },
        Command::Doctor { json } => doctor::doctor(json),
        Command::Completions { shell } => completions::print_completions(shell),
        Command::Man => completions::print_man(),
        #[cfg(windows)]
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
                autostart: !no_logon_task,
            },
        }),
        #[cfg(not(windows))]
        Command::Install {
            from,
            no_claude,
            no_autostart,
        } => install::install(install::Options {
            from: from.as_deref(),
            setup: registration::Setup {
                claude: !no_claude,
                autostart: !no_autostart,
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
            // `install.ps1` finds a failed install by a log line starting `Error:`.
            anstream::eprintln!("{} {error:#}", paint(BAD, "Error:"));
            if let Some(hint) = hint(&error) {
                anstream::eprintln!("hint: {hint}");
            }
            ExitCode::FAILURE
        }
    }
}

/// What to do next about a failure whose message does not say it already.
fn hint(error: &anyhow::Error) -> Option<&'static str> {
    let client = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<ClientError>())?;
    match client {
        ClientError::Unreachable { .. } | ClientError::Interrupted { .. } => {
            Some("`allot daemon start` starts allotd; `allot doctor` checks the install")
        }
        ClientError::Http { status: 401, .. } => {
            Some("allot and allotd read different tokens; `allot doctor` checks the install")
        }
        ClientError::Http { status: 404, .. } => Some("`allot leases --all` lists the lease IDs"),
        ClientError::Http { status: 409, .. } => Some(
            "`allot leases` shows each lease's state: kill takes an active lease, remove a \
             queued one",
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    fn parse(args: &[&str]) -> Result<Args, clap::Error> {
        Args::try_parse_from(args)
    }

    #[test]
    fn test_args_definition_is_valid() {
        Args::command().debug_assert();
    }

    #[test]
    fn test_json_flags_parse_on_every_read_command() {
        for args in [
            &["allot", "status", "--json"][..],
            &["allot", "leases", "--all", "--json"],
            &["allot", "history", "--json"],
            &["allot", "history", "cargo build|/app", "--json"],
            &["allot", "kill", "3", "--json"],
            &["allot", "remove", "3", "--json"],
            &["allot", "daemon", "status", "--json"],
            &["allot", "doctor", "--json"],
        ] {
            assert!(parse(args).is_ok(), "{args:?}");
        }
    }

    #[test]
    fn test_status_watch_takes_an_interval_and_refuses_json() {
        let Command::Status {
            watch, interval, ..
        } = parse(&["allot", "status", "--watch", "--interval", "5"])
            .unwrap()
            .command
        else {
            panic!("not status");
        };
        assert!(watch);
        assert_eq!(interval, 5);
        assert!(parse(&["allot", "status", "--watch", "--json"]).is_err());
        assert!(parse(&["allot", "status", "--interval", "5"]).is_err());
        assert!(parse(&["allot", "status", "--watch", "--interval", "0"]).is_err());
    }

    #[test]
    fn test_color_is_global_and_defaults_to_auto() {
        assert_eq!(parse(&["allot", "status"]).unwrap().color, ColorWhen::Auto);
        assert_eq!(
            parse(&["allot", "leases", "--color", "never"])
                .unwrap()
                .color,
            ColorWhen::Never
        );
        assert!(parse(&["allot", "--color", "pink", "status"]).is_err());
    }

    #[test]
    fn test_completions_takes_every_shell() {
        for shell in ["bash", "elvish", "fish", "powershell", "zsh"] {
            assert!(parse(&["allot", "completions", shell]).is_ok(), "{shell}");
        }
        assert!(parse(&["allot", "completions", "tcsh"]).is_err());
    }

    #[test]
    fn test_help_shows_examples() {
        let help = Args::command().render_help().to_string();
        assert!(help.contains("Examples:\n  allot status "), "{help}");
        let mut command = Args::command();
        let status = command.find_subcommand_mut("status").unwrap();
        assert!(
            status
                .render_help()
                .to_string()
                .contains("allot status --watch"),
            "status help lacks its examples"
        );
    }

    #[test]
    fn test_hint_names_the_next_command() {
        let unreachable = anyhow::Error::new(ClientError::Unreachable {
            url: "http://127.0.0.1:1".into(),
            reason: "refused".into(),
        });
        assert!(hint(&unreachable).unwrap().contains("allot daemon start"));
        let missing = anyhow::Error::new(ClientError::Http {
            status: 404,
            message: "no lease 9".into(),
        })
        .context("killing lease 9");
        assert!(hint(&missing).unwrap().contains("allot leases"));
        assert_eq!(hint(&anyhow::anyhow!("other")), None);
    }
}
