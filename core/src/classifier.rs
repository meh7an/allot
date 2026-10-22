//! Decides from a shell command line whether it is heavy and which lease class it needs.
//!
//! The line is split into simple commands the way bash or PowerShell would, approximately:
//! quotes and escapes are honored, and `&& || ; | &`, newlines, parentheses, braces, `$(`
//! and bash backticks all separate commands. Each simple command is unwrapped (`sudo`,
//! `time`, `env`, `npx`, `uv run`, ...), nested shells (`bash -c`, `cmd /c`, `pwsh -Command`)
//! are classified recursively, and the result is matched against the configured rules.

use rh_shared::{
    config::{ClassifierConfig, HeavyRule},
    types::JobClass,
};

const PROGRAM_EXTENSIONS: [&str; 6] = [".exe", ".cmd", ".bat", ".com", ".ps1", ".sh"];
const PACKAGE_MANAGERS: [&str; 4] = ["npm", "pnpm", "yarn", "bun"];
const SCRIPT_RUNNERS: [&str; 4] = ["uv", "poetry", "pipenv", "pdm"];

// Nested shells deeper than this are not looked into; real commands never get close.
const MAX_NESTING: usize = 4;

/// The shell a command line is written for. Quoting and escapes differ between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Shell {
    /// bash and other POSIX shells: `\` escapes.
    Bash,
    /// PowerShell, also used for `cmd /c` scripts: `` ` `` escapes and `\` is literal.
    PowerShell,
}

/// The verdict for a command line.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Classification {
    /// Needs no lease.
    Light,
    /// Needs a lease of `class`.
    Heavy {
        /// Combined class of every heavy part: one class stays itself, CPU plus GPU is mixed.
        class: JobClass,
        /// The rule the first heavy part matched.
        matched_rule: HeavyRule,
        /// The first heavy simple command, words joined by spaces.
        command: String,
    },
}

/// Classifies command lines against the rules of a [`ClassifierConfig`].
#[derive(Debug, Clone)]
pub struct Classifier {
    rules: Vec<CompiledRule>,
    light_flags: Vec<String>,
}

#[derive(Debug, Clone)]
struct CompiledRule {
    program: String,
    args: Vec<String>,
    rule: HeavyRule,
}

#[derive(Debug)]
struct Hit {
    class: JobClass,
    rule: HeavyRule,
    command: String,
}

enum Unwrapped<'a> {
    Command(&'a [String]),
    Nested(String, Shell),
    Empty,
}

impl Classifier {
    /// Prepares the rules of `config` for matching.
    pub fn new(config: &ClassifierConfig) -> Self {
        let rules = config
            .rules
            .iter()
            .map(|rule| CompiledRule {
                program: rule.program.to_lowercase(),
                args: rule.args.iter().map(|arg| arg.to_lowercase()).collect(),
                rule: rule.clone(),
            })
            .collect();
        Self {
            rules,
            light_flags: config.light_flags.clone(),
        }
    }

    /// Classifies `line` as written for `shell`.
    pub fn classify(&self, line: &str, shell: Shell) -> Classification {
        let mut hits = Vec::new();
        self.collect_hits(line, shell, 0, &mut hits);
        let Some(first) = hits.first() else {
            return Classification::Light;
        };
        let class = hits.iter().map(|hit| hit.class).fold(first.class, combine);
        Classification::Heavy {
            class,
            matched_rule: first.rule.clone(),
            command: first.command.clone(),
        }
    }

    fn collect_hits(&self, line: &str, shell: Shell, depth: usize, hits: &mut Vec<Hit>) {
        if depth > MAX_NESTING {
            return;
        }
        for words in split_commands(line, shell) {
            match unwrap(&words) {
                Unwrapped::Nested(script, nested_shell) => {
                    self.collect_hits(&script, nested_shell, depth + 1, hits);
                }
                Unwrapped::Command(command) => {
                    if let Some(rule) = self.matching_rule(command) {
                        hits.push(Hit {
                            class: rule.class,
                            rule: rule.clone(),
                            command: words.join(" "),
                        });
                    }
                }
                Unwrapped::Empty => {}
            }
        }
    }

    fn matching_rule(&self, command: &[String]) -> Option<&HeavyRule> {
        let (first, args) = command.split_first()?;
        if args.iter().any(|arg| self.light_flags.contains(arg)) {
            return None;
        }
        let program = program_name(first);
        let lowered: Vec<String> = args.iter().map(|arg| arg.to_lowercase()).collect();
        let positional = first_positional(&program, &lowered);
        self.rules
            .iter()
            .find(|rule| glob(&rule.program, &program) && rule.matches_args(&lowered, positional))
            .map(|rule| &rule.rule)
    }
}

impl CompiledRule {
    fn matches_args(&self, args: &[String], positional: Option<&str>) -> bool {
        self.args.is_empty()
            || self.args.iter().any(|pattern| {
                if pattern.starts_with('-') {
                    args.iter().any(|arg| glob(pattern, arg))
                } else {
                    positional
                        .is_some_and(|arg| glob(pattern, arg) || glob(pattern, file_name(arg)))
                }
            })
    }
}

/// One line per rule, for documentation: `cargo build|test -> cpu_heavy`.
pub fn describe_rule(rule: &HeavyRule) -> String {
    let args = if rule.args.is_empty() {
        "(any arguments)".to_owned()
    } else {
        rule.args.join("|")
    };
    format!("{} {args} -> {}", rule.program, rule.class.as_str())
}

fn combine(a: JobClass, b: JobClass) -> JobClass {
    match (a, b) {
        _ if a == b => a,
        (JobClass::Light, other) | (other, JobClass::Light) => other,
        _ => JobClass::Mixed,
    }
}

/// Splits `line` into simple commands, each a list of words with quotes removed.
fn split_commands(line: &str, shell: Shell) -> Vec<Vec<String>> {
    let escape = match shell {
        Shell::Bash => '\\',
        _ => '`',
    };
    let chars: Vec<char> = line.chars().collect();
    let mut commands = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut index = 0;

    let flush_word = |words: &mut Vec<String>, word: &mut String, in_word: &mut bool| {
        if *in_word {
            words.push(std::mem::take(word));
            *in_word = false;
        }
    };
    let flush_command = |commands: &mut Vec<Vec<String>>, words: &mut Vec<String>| {
        if !words.is_empty() {
            commands.push(std::mem::take(words));
        }
    };

    while index < chars.len() {
        let current = chars[index];
        let next = chars.get(index + 1).copied();
        let previous = index.checked_sub(1).map(|at| chars[at]);
        match current {
            _ if current == escape => {
                match next {
                    // A line continuation joins the next line onto this command.
                    Some('\r') if chars.get(index + 2) == Some(&'\n') => index += 1,
                    Some('\n') => {}
                    Some(escaped) => {
                        word.push(escaped);
                        in_word = true;
                    }
                    None => {}
                }
                index += 2;
                continue;
            }
            '\'' => {
                index += 1;
                while index < chars.len() && chars[index] != '\'' {
                    word.push(chars[index]);
                    index += 1;
                }
                in_word = true;
            }
            '"' => {
                index += 1;
                while index < chars.len() && chars[index] != '"' {
                    if chars[index] == escape && index + 1 < chars.len() {
                        index += 1;
                    }
                    word.push(chars[index]);
                    index += 1;
                }
                in_word = true;
            }
            '#' if !in_word => {
                while index < chars.len() && chars[index] != '\n' {
                    index += 1;
                }
                continue;
            }
            ' ' | '\t' | '\r' => flush_word(&mut words, &mut word, &mut in_word),
            // `2>&1` and `&>` are redirects, not separators.
            '&' if previous == Some('>') || next == Some('>') => {
                word.push(current);
                in_word = true;
            }
            '$' if next == Some('(') => {
                flush_word(&mut words, &mut word, &mut in_word);
                flush_command(&mut commands, &mut words);
                index += 1;
            }
            '\n' | ';' | '|' | '&' | '(' | ')' | '{' | '}' | '`' => {
                flush_word(&mut words, &mut word, &mut in_word);
                flush_command(&mut commands, &mut words);
            }
            _ => {
                word.push(current);
                in_word = true;
            }
        }
        index += 1;
    }
    flush_word(&mut words, &mut word, &mut in_word);
    flush_command(&mut commands, &mut words);
    commands
}

/// Strips environment assignments and wrapper programs, or finds the script of a nested
/// shell.
fn unwrap(words: &[String]) -> Unwrapped<'_> {
    let mut rest = skip_while(words, is_assignment);
    loop {
        let Some((first, args)) = rest.split_first() else {
            return Unwrapped::Empty;
        };
        let program = program_name(first);
        let subcommand = args.first().map(|arg| arg.to_lowercase());
        let subcommand = subcommand.as_deref();
        rest = match program.as_str() {
            "sudo" | "time" | "nohup" | "exec" | "command" | "builtin" | "call" | "npx" => {
                skip_flags(args)
            }
            "nice" | "timeout" => skip_while(skip_flags(args), starts_with_digit),
            "env" => skip_while(skip_flags(args), is_assignment),
            manager
                if PACKAGE_MANAGERS.contains(&manager)
                    && matches!(subcommand, Some("exec" | "dlx" | "x")) =>
            {
                skip_while(skip_flags(&args[1..]), |arg| arg == "--")
            }
            runner if SCRIPT_RUNNERS.contains(&runner) && subcommand == Some("run") => {
                skip_flags(&args[1..])
            }
            "bash" | "sh" | "zsh" | "dash" => {
                return match args.iter().position(|arg| arg == "-c") {
                    Some(at) => match args.get(at + 1) {
                        Some(script) => Unwrapped::Nested(script.clone(), Shell::Bash),
                        None => Unwrapped::Empty,
                    },
                    None => Unwrapped::Command(rest),
                };
            }
            "cmd" => {
                let script_flag = |arg: &String| matches!(arg.to_lowercase().as_str(), "/c" | "/k");
                return nested_after(rest, args, script_flag, Shell::PowerShell);
            }
            "powershell" | "pwsh" => {
                let command_flag = |arg: &String| {
                    let arg = arg.to_lowercase();
                    arg.len() >= 2 && "-command".starts_with(&arg)
                };
                return nested_after(rest, args, command_flag, Shell::PowerShell);
            }
            _ => return Unwrapped::Command(rest),
        };
    }
}

/// The words after the first `flag` in `args`, joined into a script for `shell`; the
/// command itself when no such flag is present.
fn nested_after<'a>(
    command: &'a [String],
    args: &[String],
    flag: impl Fn(&String) -> bool,
    shell: Shell,
) -> Unwrapped<'a> {
    match args.iter().position(flag) {
        Some(at) => Unwrapped::Nested(args[at + 1..].join(" "), shell),
        None => Unwrapped::Command(command),
    }
}

fn skip_while(words: &[String], skip: impl Fn(&str) -> bool) -> &[String] {
    let skipped = words.iter().take_while(|word| skip(word)).count();
    &words[skipped..]
}

fn skip_flags(words: &[String]) -> &[String] {
    skip_while(words, |word| word.starts_with('-'))
}

fn starts_with_digit(word: &str) -> bool {
    word.starts_with(|c: char| c.is_ascii_digit())
}

/// `NAME=value`, as in `RUST_LOG=debug cargo run`.
fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// The first argument not starting with `-` or `+`. For package managers, `run` and
/// `run-script` are skipped so `pnpm run build` reads like `pnpm build`.
fn first_positional<'a>(program: &str, args: &'a [String]) -> Option<&'a str> {
    let mut positional = args
        .iter()
        .filter(|arg| !arg.starts_with('-') && !arg.starts_with('+'))
        .map(String::as_str);
    let first = positional.next()?;
    if PACKAGE_MANAGERS.contains(&program) && matches!(first, "run" | "run-script") {
        return positional.next();
    }
    Some(first)
}

/// Lowercase file name of a program path without its extension: `C:\bin\Cargo.EXE` is
/// `cargo`.
fn program_name(word: &str) -> String {
    let name = file_name(word).to_lowercase();
    PROGRAM_EXTENSIONS
        .iter()
        .find_map(|extension| name.strip_suffix(extension).filter(|stem| !stem.is_empty()))
        .map_or_else(|| name.clone(), str::to_owned)
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Matches `text` against `pattern`, where `*` stands for any run of characters.
fn glob(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0, 0);
    let mut backtrack: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && pattern[p] == '*' {
            backtrack = Some((p, t));
            p += 1;
        } else if p < pattern.len() && pattern[p] == text[t] {
            p += 1;
            t += 1;
        } else if let Some((star, matched)) = backtrack {
            p = star + 1;
            t = matched + 1;
            backtrack = Some((star, matched + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    use JobClass::{CpuHeavy, GpuHeavy, Mixed};
    use Shell::{Bash, PowerShell};

    fn classifier() -> Classifier {
        Classifier::new(&ClassifierConfig::default())
    }

    fn heavy_class(verdict: &Classification) -> Option<JobClass> {
        match verdict {
            Classification::Heavy { class, .. } => Some(*class),
            _ => None,
        }
    }

    fn class_of(line: &str, shell: Shell) -> Option<JobClass> {
        heavy_class(&classifier().classify(line, shell))
    }

    const CPU: Option<JobClass> = Some(CpuHeavy);
    const GPU: Option<JobClass> = Some(GpuHeavy);
    const MIXED: Option<JobClass> = Some(Mixed);
    const LIGHT: Option<JobClass> = None;

    const BASH_CASES: &[(&str, Option<JobClass>)] = &[
        ("cargo build", CPU),
        ("cargo build --release", CPU),
        ("cargo +nightly test -p rh-core", CPU),
        ("cd core && cargo test", CPU),
        ("cargo check", LIGHT),
        ("cargo clippy --all-targets -- -D warnings", LIGHT),
        ("cargo fmt --check", LIGHT),
        ("cargo build --help", LIGHT),
        ("cargo --version", LIGHT),
        ("RUST_LOG=debug cargo run --bin rhd", CPU),
        ("FOO=1 BAR=2 cargo bench", CPU),
        ("env RUSTFLAGS=-Ctarget-cpu=native cargo build", CPU),
        ("time cargo test 2>&1 | tail -5", CPU),
        ("timeout 600 cargo test --workspace", CPU),
        ("nice -n 10 cargo build", CPU),
        ("sudo time cargo build", CPU),
        ("git status && git diff", LIGHT),
        ("echo \"cargo build\"", LIGHT),
        ("git commit -m 'run cargo build later'", LIGHT),
        ("ls -la; cargo build; echo done", CPU),
        ("(cd sampler && cargo build)", CPU),
        ("bash -c \"cd core && cargo test\"", CPU),
        ("sh -c 'pnpm build'", CPU),
        ("pnpm build", CPU),
        ("pnpm run build", CPU),
        ("pnpm test -- --watch=false", CPU),
        ("pnpm install", LIGHT),
        ("pnpm run lint", LIGHT),
        ("npx pnpm build", CPU),
        ("cmake --build build --config Release", CPU),
        ("cmake -S . -B build", LIGHT),
        ("python train.py --epochs 3", GPU),
        ("python3 -u scripts/train_gpt.py", GPU),
        ("uv run python train_lora.py", GPU),
        ("python -m pytest", LIGHT),
        ("cargo build # then test", CPU),
        ("# cargo build", LIGHT),
        ("cargo build && python train.py", MIXED),
        ("echo $(cargo build 2>&1)", CPU),
        ("echo `cargo build`", CPU),
        ("cargo build \\\n  --release", CPU),
        ("cargo build > build.log 2>&1 &", CPU),
        ("/c/Users/me/.cargo/bin/cargo.exe build", CPU),
        ("command -v cargo", LIGHT),
        ("powershell -c cargo bench", CPU),
    ];

    const POWERSHELL_CASES: &[(&str, Option<JobClass>)] = &[
        ("cargo build --release", CPU),
        ("Set-Location core; cargo test", CPU),
        ("& \"C:\\CMake\\bin\\cmake.exe\" --build out", CPU),
        ("$env:RUST_LOG = \"debug\"; cargo run", CPU),
        ("Get-ChildItem | Select-Object Name", LIGHT),
        ("Measure-Command { cargo build }", CPU),
        ("cmd /c \"cd core && cargo build\"", CPU),
        ("pwsh -NoProfile -Command \"cargo test\"", CPU),
        ("msbuild App.sln /m /p:Configuration=Release", CPU),
        ("MSBuild.exe /?", LIGHT),
        ("C:\\Python312\\python.exe train.py", GPU),
        ("Write-Output \"cargo build\"", LIGHT),
        ("cargo test `\n  --workspace", CPU),
        ("py -3.12 train.py", GPU),
        ("git log --oneline -5", LIGHT),
        ("cargo build 2>&1 | Out-File build.log", CPU),
        ("if ($?) { pnpm test }", CPU),
        ("", LIGHT),
    ];

    #[test]
    fn test_classify_real_commands_table() {
        assert!(BASH_CASES.len() + POWERSHELL_CASES.len() >= 40);
        let tables = [(Bash, BASH_CASES), (PowerShell, POWERSHELL_CASES)];
        for (shell, cases) in tables {
            for &(line, expected) in cases {
                assert_eq!(class_of(line, shell), expected, "{shell:?}: {line:?}");
            }
        }
    }

    #[test]
    fn test_classify_reports_first_heavy_rule_and_command() {
        let verdict = classifier().classify("cd x && cargo test --workspace; pnpm build", Bash);
        let Classification::Heavy {
            class,
            matched_rule,
            command,
        } = verdict
        else {
            panic!("expected heavy, got {verdict:?}");
        };
        assert_eq!(class, CpuHeavy);
        assert_eq!(matched_rule.program, "cargo");
        assert_eq!(command, "cargo test --workspace");
    }

    #[test]
    fn test_classify_custom_rules_and_light_flags() {
        let config = ClassifierConfig {
            rules: vec![HeavyRule {
                program: "EldenRing".into(),
                args: vec![],
                class: GpuHeavy,
            }],
            light_flags: vec!["--dry-run".into()],
        };
        let classifier = Classifier::new(&config);
        let heavy = classifier.classify("& 'D:\\Games\\eldenring.exe'", PowerShell);
        assert_eq!(heavy_class(&heavy), GPU);
        let dry = classifier.classify("eldenring --dry-run", Bash);
        assert_eq!(dry, Classification::Light);
        let cargo = classifier.classify("cargo build", Bash);
        assert_eq!(cargo, Classification::Light);
    }

    #[test]
    fn test_classify_nesting_is_bounded() {
        let nested = |levels: usize| format!("{}cargo build", "cmd /c ".repeat(levels));
        let deepest = classifier().classify(&nested(MAX_NESTING), PowerShell);
        assert!(matches!(deepest, Classification::Heavy { .. }));
        let too_deep = classifier().classify(&nested(MAX_NESTING + 1), PowerShell);
        assert_eq!(too_deep, Classification::Light);
    }

    #[test]
    fn test_split_commands_honors_quotes_and_escapes() {
        let bash = split_commands("echo 'a;b' \"c\\\"d\" e\\ f; ls", Bash);
        assert_eq!(bash, [vec!["echo", "a;b", "c\"d", "e f"], vec!["ls"]]);
        let pwsh = split_commands("Write-Host \"a`\"b\" C:\\x | sort", PowerShell);
        assert_eq!(pwsh, [vec!["Write-Host", "a\"b", "C:\\x"], vec!["sort"]]);
        let empty_quotes = split_commands("printf '' x", Bash);
        assert_eq!(empty_quotes, [vec!["printf", "", "x"]]);
    }

    #[test]
    fn test_combine_classes() {
        assert_eq!(combine(CpuHeavy, CpuHeavy), CpuHeavy);
        assert_eq!(combine(CpuHeavy, GpuHeavy), Mixed);
        assert_eq!(combine(JobClass::Light, GpuHeavy), GpuHeavy);
        assert_eq!(combine(Mixed, CpuHeavy), Mixed);
    }

    #[test]
    fn test_glob_wildcards() {
        assert!(glob("train*.py", "train_gpt.py"));
        assert!(glob("python*", "python"));
        assert!(glob("*", ""));
        assert!(glob("a*b*c", "axxbyyc"));
        assert!(!glob("train*.py", "pretrain.py"));
        assert!(!glob("py", "python"));
    }

    #[test]
    fn test_program_name_strips_directory_and_extension() {
        assert_eq!(program_name("C:\\bin\\Cargo.EXE"), "cargo");
        assert_eq!(program_name("/usr/bin/python3"), "python3");
        assert_eq!(program_name("build.sh"), "build");
        assert_eq!(program_name(".exe"), ".exe");
    }

    #[test]
    fn test_describe_rule_lists_args_and_class() {
        let rules = ClassifierConfig::default().rules;
        assert_eq!(
            describe_rule(&rules[0]),
            "cargo build|test|run|bench -> cpu_heavy"
        );
        assert_eq!(
            describe_rule(&rules[2]),
            "msbuild (any arguments) -> cpu_heavy"
        );
    }
}
