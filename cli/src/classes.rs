//! `allot classes`: the heavy-command rules and class profiles in effect, as Markdown.

use std::fmt::Write;

use allot_core::classifier::describe_rule;
use allot_shared::{
    config::Config,
    paths::Paths,
    types::{JobClass, JobProfile},
};

/// Prints the rules, class profiles, and presets from the user's config, or the defaults
/// without one.
///
/// # Errors
///
/// Fails when the home directory is unknown or the config file is invalid.
pub fn classes() -> anyhow::Result<()> {
    let paths = Paths::from_env()?;
    let config = Config::load(&paths.config_file())?;
    print!("{}", format_classes(&config));
    Ok(())
}

/// Renders `config`'s classifier rules, light flags, class profiles, and presets.
///
/// The `resource-guard` skill embeds this output for the default config; a test keeps the
/// two identical.
fn format_classes(config: &Config) -> String {
    let mut out = String::from(
        "A command line is heavy when any command in it (after `&&`, `;`, `|`, env prefixes, \
         and wrappers such as `npx` or `bash -c`) matches a rule:\n\n",
    );
    for rule in &config.classifier.rules {
        // Writing to a String cannot fail.
        let _ = writeln!(out, "- `{}`", describe_rule(rule));
    }
    let flags: Vec<String> = config
        .classifier
        .light_flags
        .iter()
        .map(|flag| format!("`{flag}`"))
        .collect();
    let _ = writeln!(
        out,
        "\nA command with any of {} is light whatever it runs.\n",
        flags.join(" ")
    );
    out.push_str(
        "| Class | Cores | RAM MB | GPU % | VRAM MB | Est. secs |\n\
         |-------|------:|-------:|------:|--------:|----------:|\n",
    );
    for class in JobClass::ALL {
        push_row(&mut out, class.as_str(), config.classes.get(class));
    }
    out.push_str(
        "\n| Preset | Cores | RAM MB | GPU % | VRAM MB | Est. secs |\n\
         |--------|------:|-------:|------:|--------:|----------:|\n",
    );
    for (name, profile) in &config.presets.0 {
        push_row(&mut out, name, profile);
    }
    out
}

fn push_row(out: &mut String, name: &str, profile: &JobProfile) {
    // Writing to a String cannot fail.
    let _ = writeln!(
        out,
        "| {name} | {} | {} | {} | {} | {} |",
        profile.cores, profile.ram_mb, profile.gpu_pct, profile.vram_mb, profile.est_secs
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const SKILL: &str = include_str!("../../skill/resource-guard/SKILL.md");
    const BEGIN: &str = "<!-- allot classes: begin -->";
    const END: &str = "<!-- allot classes: end -->";

    #[test]
    fn test_skill_classes_block_matches_default_config() {
        let skill = SKILL.replace("\r\n", "\n");
        let start = skill.find(BEGIN).expect("SKILL.md lacks the begin marker") + BEGIN.len();
        let end = skill.find(END).expect("SKILL.md lacks the end marker");
        let expected = format_classes(&Config::default());
        assert_eq!(
            skill[start..end].trim(),
            expected.trim(),
            "skill/resource-guard/SKILL.md is out of date; replace its classes block with:\n\n\
             {expected}"
        );
    }

    #[test]
    fn test_format_classes_shows_user_rules_and_profiles() {
        let config = Config::from_toml(
            "[[classifier.rules]]\nprogram = \"ninja\"\nclass = \"cpu_heavy\"\n\n\
             [classes.cpu_heavy]\ncores = 12.0\n",
        )
        .unwrap();
        let out = format_classes(&config);
        assert!(
            out.contains("- `ninja (any arguments) -> cpu_heavy`"),
            "{out}"
        );
        assert!(!out.contains("cargo"), "{out}");
        assert!(
            out.contains("| cpu_heavy | 12 | 4096 | 0 | 0 | 300 |"),
            "{out}"
        );
    }
}
