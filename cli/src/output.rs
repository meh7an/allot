//! How the commands print: colour that follows `--color`, then `NO_COLOR`, `CLICOLOR_FORCE`, and
//! whether the stream is a terminal, and the `--json` form of the read commands.

use std::ffi::OsString;

use anstream::ColorChoice;
use anstyle::{AnsiColor, Style};
use serde::Serialize;

/// Section headings, such as `sessions`.
pub const HEADING: Style = Style::new().bold();
/// Details that help tell lines apart but are not the point, such as session IDs.
pub const DIM: Style = Style::new().dimmed();
/// Running, granted, or fine.
pub const GOOD: Style = AnsiColor::Green.on_default();
/// Waiting, or worth a look.
pub const WAIT: Style = AnsiColor::Yellow.on_default();
/// Stopped by the user, rejected, or failed.
pub const BAD: Style = AnsiColor::Red.on_default().bold();

/// The `--color` flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ColorWhen {
    /// Colour on a terminal, unless `NO_COLOR` is set.
    Auto,
    /// Colour even when piped, as `CLICOLOR_FORCE` does.
    Always,
    /// No colour.
    Never,
}

impl ColorWhen {
    /// Makes every later write through `anstream`, including clap's help and errors, follow
    /// this choice.
    pub fn apply(self) {
        let choice = match self {
            Self::Auto => ColorChoice::Auto,
            Self::Always => ColorChoice::Always,
            Self::Never => ColorChoice::Never,
        };
        choice.write_global();
    }
}

/// The `--color` value among `args`, read before clap parses them, so that clap's own help and
/// errors follow it too. The last one wins, as clap takes it; `None` without one or with a value
/// clap will reject.
pub fn color_in_args(args: impl IntoIterator<Item = OsString>) -> Option<ColorWhen> {
    let mut found = None;
    let mut args = args.into_iter().skip(1);
    while let Some(arg) = args.next() {
        let Some(arg) = arg.to_str() else {
            continue;
        };
        let value = match arg {
            "--" => break,
            "--color" => args.next().and_then(|value| value.into_string().ok()),
            _ => arg.strip_prefix("--color=").map(str::to_owned),
        };
        if let Some(value) = value {
            found = <ColorWhen as clap::ValueEnum>::from_str(&value, false).ok();
        }
    }
    found
}

/// `text` in `style`, with the style reset after it.
pub fn paint(style: Style, text: &str) -> String {
    format!("{style}{text}{style:#}")
}

/// Prints `value` as pretty JSON with a final newline: the daemon's wire format, unchanged.
///
/// # Errors
///
/// Fails when the value cannot be serialized, which the wire types never do.
pub fn print_json<T: Serialize>(value: &T) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

/// `text` as a pipe or `NO_COLOR` gets it, for tests of coloured output.
#[cfg(test)]
pub fn plain(text: String) -> String {
    anstream::adapter::strip_str(&text).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn test_color_in_args_reads_both_forms_and_the_last_wins() {
        assert_eq!(
            color_in_args(args(&["allot", "--color", "never", "status"])),
            Some(ColorWhen::Never)
        );
        assert_eq!(
            color_in_args(args(&["allot", "status", "--color=always"])),
            Some(ColorWhen::Always)
        );
        assert_eq!(
            color_in_args(args(&["allot", "--color=never", "--color", "auto"])),
            Some(ColorWhen::Auto)
        );
    }

    #[test]
    fn test_color_in_args_ignores_a_bad_value_and_what_follows_a_double_dash() {
        assert_eq!(color_in_args(args(&["allot", "--color", "pink"])), None);
        assert_eq!(color_in_args(args(&["allot", "--", "--color=never"])), None);
        assert_eq!(color_in_args(args(&["allot", "status"])), None);
    }

    #[test]
    fn test_paint_wraps_the_text_in_the_style_and_a_reset() {
        let painted = paint(BAD, "killed");
        assert!(painted.starts_with("\u{1b}["), "{painted:?}");
        assert!(painted.ends_with("\u{1b}[0m"), "{painted:?}");
        assert_eq!(plain(painted), "killed");
    }
}
