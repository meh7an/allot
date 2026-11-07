//! `allot completions <shell>` and `allot man`, which print what packages ship, and the
//! completion files `allot install` writes where bash and fish load them by themselves. zsh and
//! PowerShell only load what a profile names, and Allot never edits a profile, so for those it
//! prints the line to add.

use std::io;
#[cfg(unix)]
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use allot_shared::paths::Paths;
use clap::CommandFactory;
use clap_complete::Shell;

#[cfg(unix)]
use crate::{install::remove_if_present, xdg};

const BIN: &str = "allot";
/// The first line of every file `allot install` writes, so that `allot uninstall` removes only
/// those and an install never overwrites one the user wrote.
#[cfg(any(unix, test))]
const MARKER: &str = "# Written by `allot install`; `allot uninstall` removes it.\n";
#[cfg(unix)]
const BASH_USER_DIR_VAR: &str = "BASH_COMPLETION_USER_DIR";

/// Prints the completion script for `shell`.
///
/// # Errors
///
/// Fails when stdout cannot be written.
pub fn print_completions(shell: Shell) -> anyhow::Result<()> {
    io::Write::write_all(&mut io::stdout(), script(shell).as_bytes())?;
    Ok(())
}

/// Prints the man page, `allot.1`, as roff.
///
/// # Errors
///
/// Fails when stdout cannot be written.
pub fn print_man() -> anyhow::Result<()> {
    io::Write::write_all(&mut io::stdout(), &man_page()?)?;
    Ok(())
}

fn script(shell: Shell) -> String {
    let mut out = Vec::new();
    clap_complete::generate(shell, &mut crate::Args::command(), BIN, &mut out);
    String::from_utf8_lossy(&out).into_owned()
}

fn man_page() -> anyhow::Result<Vec<u8>> {
    let mut out = Vec::new();
    clap_mangen::Man::new(crate::Args::command()).render(&mut out)?;
    Ok(out)
}

/// Where bash-completion and fish load a user's completions from, for the shells
/// [`install`] writes.
#[cfg(unix)]
fn completion_files(paths: &Paths) -> [(Shell, PathBuf); 2] {
    let bash_dir = env::var_os(BASH_USER_DIR_VAR)
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| xdg::data_home(paths).join("bash-completion"));
    [
        (Shell::Bash, bash_dir.join("completions").join(BIN)),
        (
            Shell::Fish,
            xdg::config_home(paths)
                .join("fish")
                .join("completions")
                .join(format!("{BIN}.fish")),
        ),
    ]
}

/// Writes the bash and fish completions where those shells load them, and says how to load the
/// zsh ones. A file there that `allot install` did not write is left alone. Failures are
/// reported, not returned: completions are a convenience.
#[cfg(unix)]
pub fn install(paths: &Paths) {
    for (shell, file) in completion_files(paths) {
        match write_owned(&file, &script(shell)) {
            Ok(true) => println!("wrote the {shell} completions to {}", file.display()),
            Ok(false) => println!(
                "left {} alone: allot install did not write it",
                file.display()
            ),
            Err(error) => println!("warning: no {shell} completions ({error:#})"),
        }
    }
    println!(
        "zsh completions: run `allot completions zsh > ~/.zfunc/_allot` and put \
         `fpath+=~/.zfunc` before `compinit` in ~/.zshrc"
    );
}

/// Says how to load the PowerShell completions, which only a profile can do.
#[cfg(windows)]
pub fn install() {
    println!(
        "PowerShell completions: add `allot completions powershell | Out-String | \
         Invoke-Expression` to your $PROFILE"
    );
}

/// Removes the completion files [`install`] wrote.
///
/// # Errors
///
/// Fails when a file of ours cannot be deleted.
#[cfg(unix)]
pub fn uninstall(paths: &Paths) -> anyhow::Result<()> {
    for (shell, file) in completion_files(paths) {
        if fs::read_to_string(&file).is_ok_and(|text| is_ours(&text)) {
            remove_if_present(&file)?;
            println!("removed the {shell} completions {}", file.display());
        }
    }
    Ok(())
}

/// Writes `script` with [`MARKER`] to `file` unless a file there lacks the marker. Returns
/// whether it wrote.
#[cfg(unix)]
fn write_owned(file: &Path, script: &str) -> anyhow::Result<bool> {
    use anyhow::Context;

    match fs::read_to_string(file) {
        Ok(text) if !is_ours(&text) => return Ok(false),
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            return Err(error).with_context(|| format!("reading {}", file.display()));
        }
        _ => {}
    }
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    fs::write(file, format!("{MARKER}{script}"))
        .with_context(|| format!("writing {}", file.display()))?;
    Ok(true)
}

#[cfg(any(unix, test))]
fn is_ours(text: &str) -> bool {
    text.starts_with(MARKER)
}

#[cfg(test)]
mod tests {
    use clap::ValueEnum;

    use super::*;

    #[test]
    fn test_script_for_every_shell_names_the_subcommands() {
        for shell in Shell::value_variants() {
            let text = script(*shell);
            for command in ["status", "leases", "doctor", "completions"] {
                assert!(text.contains(command), "{shell} lacks {command}");
            }
        }
    }

    #[test]
    fn test_script_completes_the_color_values() {
        let text = script(Shell::Fish);
        assert!(text.contains("-l color"), "{text}");
        assert!(text.contains("always"), "{text}");
    }

    #[test]
    fn test_man_page_is_roff_with_the_commands() {
        let page = String::from_utf8(man_page().unwrap()).unwrap();
        assert!(page.contains(".TH allot"), "{page}");
        assert!(page.contains("doctor"), "{page}");
        assert!(page.contains("Examples:"), "{page}");
    }

    #[test]
    fn test_is_ours_needs_the_marker_on_the_first_line() {
        assert!(is_ours(&format!("{MARKER}complete -c allot")));
        assert!(!is_ours("# my own allot completions\n"));
        assert!(!is_ours(&format!("\n{MARKER}")));
    }

    #[cfg(unix)]
    #[test]
    fn test_write_owned_replaces_its_own_file_and_keeps_the_users() {
        let dir = env::temp_dir().join(format!("allot-completions-{}", std::process::id()));
        let file = dir.join("completions").join("allot");
        assert!(write_owned(&file, "first\n").unwrap());
        assert!(write_owned(&file, "second\n").unwrap());
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            format!("{MARKER}second\n")
        );
        fs::write(&file, "mine\n").unwrap();
        assert!(!write_owned(&file, "third\n").unwrap());
        assert_eq!(fs::read_to_string(&file).unwrap(), "mine\n");
        fs::remove_dir_all(&dir).unwrap();
    }
}
