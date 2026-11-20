//! The config writer on real text and real files: round-trip fixtures (every edit leaves the
//! rest of the file byte-identical, and the file then reads as the old config with exactly the
//! intended change), invalid changes refused with the file untouched, conflicts with edits
//! saved by hand, and files held open by other programs.
//!
//! Expected outputs live in `tests/config_write/expected/`. `ALLOT_BLESS=1` rewrites them;
//! review the diff against the fixture before keeping a blessed file.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use allot_shared::{
    config::{Config, HeavyRule, Presets, Sizing},
    config_write::{self, Change, Revision, WriteError},
    types::{JobClass, JobProfile},
};
use tempfile::TempDir;

struct Case {
    name: &'static str,
    fixture: &'static str,
    changes: fn() -> Vec<Change>,
    expect: fn(&mut Config),
}

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("config_write")
        .join(name)
}

fn fixture(name: &str) -> String {
    fs::read_to_string(dir("fixtures").join(name)).unwrap()
}

fn rule(program: &str, args: &[&str], class: JobClass) -> HeavyRule {
    HeavyRule {
        program: program.into(),
        args: args.iter().map(|&arg| arg.into()).collect(),
        class,
    }
}

fn number(key: &str, value: f64) -> Change {
    Change::SetNumber {
        key: key.into(),
        value,
    }
}

fn switch(key: &str, value: bool) -> Change {
    Change::SetSwitch {
        key: key.into(),
        value,
    }
}

fn reset(key: &str) -> Change {
    Change::Reset { key: key.into() }
}

fn profile(cores: f32, ram_mb: u64, gpu_pct: f32, vram_mb: u64, est_secs: u64) -> JobProfile {
    JobProfile {
        cores,
        ram_mb,
        gpu_pct,
        vram_mb,
        est_secs,
    }
}

fn set_row(table: &str, name: &str, profile: JobProfile, min_cores: Option<f32>) -> Change {
    Change::SetRow {
        table: table.into(),
        name: name.into(),
        profile,
        min_cores,
    }
}

fn reset_row(table: &str, name: &str) -> Change {
    Change::ResetRow {
        table: table.into(),
        name: name.into(),
    }
}

fn default_preset(name: &str) -> Sizing {
    Presets::default().0.remove(name).unwrap()
}

/// The rules in `hand_edited.toml`, in file order.
fn hand_edited_rules() -> [HeavyRule; 3] {
    [
        rule(
            "cargo",
            &["build", "test", "run", "bench", "nextest"],
            JobClass::CpuHeavy,
        ),
        rule("pnpm", &["build*", "test*"], JobClass::CpuHeavy),
        rule("blender", &[], JobClass::GpuHeavy),
    ]
}

/// The rules in `inline_rules.toml`, in file order.
fn inline_rules() -> [HeavyRule; 3] {
    [
        rule("cargo", &["build", "test"], JobClass::CpuHeavy),
        rule("make", &[], JobClass::CpuHeavy),
        rule("python*", &["train*.py"], JobClass::GpuHeavy),
    ]
}

/// pnpm and cargo swapped, blender dropped, msbuild added between them.
fn reordered_hand_rules() -> Vec<HeavyRule> {
    let [cargo, pnpm, _] = hand_edited_rules();
    vec![pnpm, rule("msbuild", &[], JobClass::CpuHeavy), cargo]
}

fn appended_hand_rules() -> Vec<HeavyRule> {
    let [cargo, pnpm, blender] = hand_edited_rules();
    vec![cargo, pnpm, blender, rule("ninja", &[], JobClass::CpuHeavy)]
}

/// make dropped, ninja added at the end.
fn rewritten_inline_rules() -> Vec<HeavyRule> {
    let [cargo, _, python] = inline_rules();
    vec![cargo, python, rule("ninja", &["-j*"], JobClass::CpuHeavy)]
}

/// The `render` preset of `hand_edited.toml`.
fn render() -> JobProfile {
    profile(6.0, 8_192, 60.0, 6_144, 900)
}

const HAND: &str = "hand_edited.toml";
const INLINE: &str = "inline_rules.toml";

const CASES: &[Case] = &[
    Case {
        name: "scalar_change",
        fixture: HAND,
        changes: || vec![number("admission.headroom_pct", 30.0)],
        expect: |config| config.admission.headroom_pct = 30.0,
    },
    Case {
        name: "scalar_bool_change",
        fixture: HAND,
        changes: || vec![switch("estimates.lower", true)],
        expect: |config| config.estimates.lower = true,
    },
    Case {
        name: "scalar_new_key",
        fixture: HAND,
        changes: || vec![number("estimates.min_runs", 5.0)],
        expect: |config| config.estimates.min_runs = 5,
    },
    Case {
        name: "scalar_new_table",
        fixture: HAND,
        changes: || vec![number("benchmark.quiet_secs", 20.0)],
        expect: |config| config.benchmark.quiet_secs = 20,
    },
    Case {
        name: "scalar_remove_empties_table",
        fixture: HAND,
        changes: || vec![reset("estimates.lower")],
        expect: |config| config.estimates.lower = true,
    },
    Case {
        name: "scalar_remove_keeps_commented_table",
        fixture: HAND,
        changes: || vec![reset("daemon.port")],
        expect: |_| {},
    },
    Case {
        name: "list_change",
        fixture: HAND,
        changes: || {
            vec![Change::SetList {
                key: "classifier.light_flags".into(),
                value: vec!["--help".into(), "-h".into(), "-V".into()],
            }]
        },
        expect: |config| {
            config.classifier.light_flags = ["--help", "-h", "-V"].map(String::from).to_vec();
        },
    },
    Case {
        name: "class_new_table",
        fixture: HAND,
        changes: || {
            let gpu_heavy = profile(2.0, 4_096, 80.0, 8_192, 600);
            vec![set_row("classes", "gpu_heavy", gpu_heavy, None)]
        },
        expect: |config| config.classes.gpu_heavy.profile.vram_mb = 8_192,
    },
    Case {
        name: "preset_new_table",
        fixture: HAND,
        changes: || {
            let big = profile(12.0, 16_384, 0.0, 0, 300);
            vec![set_row("presets", "big", big, Some(4.0))]
        },
        expect: |config| {
            let sizing = Sizing {
                profile: profile(12.0, 16_384, 0.0, 0, 300),
                min_cores: Some(4.0),
            };
            config.presets.0.insert("big".into(), sizing);
        },
    },
    Case {
        name: "preset_inline_min_cores",
        fixture: HAND,
        changes: || {
            let quick = profile(2.0, 1_024, 0.0, 0, 10);
            vec![set_row("presets", "quick", quick, Some(1.5))]
        },
        expect: |config| config.presets.0.get_mut("quick").unwrap().min_cores = Some(1.5),
    },
    Case {
        name: "preset_whole_table",
        fixture: HAND,
        changes: || {
            let render = JobProfile {
                est_secs: 300,
                ..render()
            };
            vec![set_row("presets", "render", render, Some(4.0))]
        },
        expect: |config| {
            let render = config.presets.0.get_mut("render").unwrap();
            render.min_cores = Some(4.0);
            render.profile.est_secs = 300;
        },
    },
    Case {
        name: "preset_rename",
        fixture: HAND,
        changes: || {
            vec![
                reset_row("presets", "render"),
                set_row("presets", "render_hq", render(), Some(3.0)),
            ]
        },
        expect: |config| {
            let render = config.presets.0.remove("render").unwrap();
            config.presets.0.insert("render_hq".into(), render);
        },
    },
    Case {
        name: "preset_remove_table",
        fixture: HAND,
        changes: || vec![reset_row("presets", "cpu_small")],
        expect: |config| {
            config
                .presets
                .0
                .insert("cpu_small".into(), default_preset("cpu_small"));
        },
    },
    Case {
        name: "preset_remove_inline",
        fixture: HAND,
        changes: || vec![reset_row("presets", "quick")],
        expect: |config| {
            config
                .presets
                .0
                .insert("quick".into(), default_preset("quick"));
        },
    },
    Case {
        name: "rules_reorder_add_drop",
        fixture: HAND,
        changes: || {
            vec![Change::SetRules {
                rules: reordered_hand_rules(),
            }]
        },
        expect: |config| config.classifier.rules = reordered_hand_rules(),
    },
    Case {
        name: "rules_append",
        fixture: HAND,
        changes: || {
            vec![Change::SetRules {
                rules: appended_hand_rules(),
            }]
        },
        expect: |config| config.classifier.rules = appended_hand_rules(),
    },
    Case {
        name: "rules_empty",
        fixture: HAND,
        changes: || vec![Change::SetRules { rules: Vec::new() }],
        expect: |config| config.classifier.rules.clear(),
    },
    Case {
        name: "rules_reset",
        fixture: HAND,
        changes: || vec![reset("classifier.rules")],
        expect: |config| config.classifier.rules = Config::default().classifier.rules,
    },
    Case {
        name: "dotted_change",
        fixture: INLINE,
        changes: || vec![number("admission.headroom_pct", 10.0)],
        expect: |config| config.admission.headroom_pct = 10.0,
    },
    Case {
        name: "dotted_new_key",
        fixture: INLINE,
        changes: || vec![number("estimates.margin_pct", 5.0)],
        expect: |config| config.estimates.margin_pct = 5.0,
    },
    Case {
        name: "inline_class_min_cores",
        fixture: INLINE,
        changes: || {
            let cpu_heavy = profile(10.0, 4_096, 0.0, 0, 300);
            vec![set_row("classes", "cpu_heavy", cpu_heavy, Some(5.0))]
        },
        expect: |config| config.classes.cpu_heavy.min_cores = Some(5.0),
    },
    Case {
        name: "inline_new_table",
        fixture: INLINE,
        changes: || vec![switch("hook.fail_open", false)],
        expect: |config| config.hook.fail_open = false,
    },
    Case {
        name: "inline_rules_rewrite",
        fixture: INLINE,
        changes: || {
            vec![Change::SetRules {
                rules: rewritten_inline_rules(),
            }]
        },
        expect: |config| config.classifier.rules = rewritten_inline_rules(),
    },
    Case {
        name: "inline_rules_empty",
        fixture: INLINE,
        changes: || vec![Change::SetRules { rules: Vec::new() }],
        expect: |config| config.classifier.rules.clear(),
    },
    Case {
        name: "batch",
        fixture: INLINE,
        changes: || {
            vec![
                number("admission.headroom_pct", 12.5),
                reset("estimates.min_runs"),
                Change::SetRules {
                    rules: rewritten_inline_rules(),
                },
            ]
        },
        expect: |config| {
            config.admission.headroom_pct = 12.5;
            config.estimates.min_runs = Config::default().estimates.min_runs;
            config.classifier.rules = rewritten_inline_rules();
        },
    },
];

fn edited(case: &Case, input: &str) -> String {
    config_write::edit(input, &(case.changes)())
        .unwrap_or_else(|error| panic!("{}: {error}", case.name))
        .text
}

#[test]
fn test_fixtures_hold_their_documented_rules() {
    let hand = Config::from_toml(&fixture(HAND)).unwrap();
    assert_eq!(hand.classifier.rules, hand_edited_rules());
    let inline = Config::from_toml(&fixture(INLINE)).unwrap();
    assert_eq!(inline.classifier.rules, inline_rules());
}

#[test]
fn test_edit_without_changes_is_byte_identical() {
    for name in [HAND, INLINE] {
        let text = fixture(name);
        assert_eq!(config_write::edit(&text, &[]).unwrap().text, text, "{name}");
    }
}

#[test]
fn test_edits_match_expected_files_and_config() {
    let bless = env::var_os("ALLOT_BLESS").is_some();
    let mut failures = Vec::new();
    for case in CASES {
        let input = fixture(case.fixture);
        let output = match config_write::edit(&input, &(case.changes)()) {
            Ok(edited) => edited,
            Err(error) => {
                failures.push(format!("{}: refused: {error}", case.name));
                continue;
            }
        };

        let mut wanted = Config::from_toml(&input).unwrap();
        (case.expect)(&mut wanted);
        if output.config != wanted {
            failures.push(format!(
                "{}: config differs: {:#?}",
                case.name, output.config
            ));
        }
        match Config::from_toml(&output.text) {
            Ok(read) if read == output.config => {}
            Ok(_) => failures.push(format!("{}: file reads differently", case.name)),
            Err(error) => failures.push(format!("{}: output rejected: {error}", case.name)),
        }

        let expected_path = dir("expected").join(format!("{}.toml", case.name));
        if bless {
            fs::write(&expected_path, &output.text).unwrap();
        } else {
            match fs::read_to_string(&expected_path) {
                Ok(expected) if expected == output.text => {}
                Ok(_) => failures.push(format!("{}: bytes differ:\n{}", case.name, output.text)),
                Err(error) => failures.push(format!("{}: no expected file: {error}", case.name)),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Every case again on the fixtures saved by Notepad: CRLF endings and a byte order mark.
#[test]
fn test_edits_keep_windows_layout() {
    let windows = |text: &str| format!("\u{feff}{}", text.replace('\n', "\r\n"));
    for case in CASES {
        let output = edited(case, &windows(&fixture(case.fixture)));
        let expected = fs::read_to_string(dir("expected").join(format!("{}.toml", case.name)));
        assert_eq!(output, windows(&expected.unwrap()), "{}", case.name);
    }
}

#[test]
fn test_edit_writes_new_whole_numbers_by_type() {
    let text = config_write::edit(
        "",
        &[
            number("admission.headroom_pct", 30.0),
            number("leases.idle_release_secs", 0.0),
        ],
    )
    .unwrap()
    .text;
    assert_eq!(
        text,
        "[admission]\nheadroom_pct = 30.0\n\n[leases]\nidle_release_secs = 0\n"
    );
    // An unchanged value keeps the user's spelling.
    let kept = config_write::edit(
        "[admission]\nheadroom_pct = 30\n",
        &[number("admission.headroom_pct", 30.0)],
    );
    assert_eq!(kept.unwrap().text, "[admission]\nheadroom_pct = 30\n");
}

fn invalid(text: &str, changes: &[Change]) -> Vec<(String, String)> {
    match config_write::edit(text, changes) {
        Err(WriteError::Invalid(problems)) => problems
            .into_iter()
            .map(|problem| (problem.key, problem.message))
            .collect(),
        other => panic!("expected invalid changes, got {other:?}"),
    }
}

fn keys(problems: &[(String, String)]) -> Vec<&str> {
    problems.iter().map(|(key, _)| key.as_str()).collect()
}

#[test]
fn test_edit_names_every_out_of_range_value() {
    let problems = invalid(
        &fixture(HAND),
        &[
            number("admission.headroom_pct", 100.0),
            number("benchmark.quiet_secs", 0.0),
            set_row("presets", "render", render(), Some(7.0)),
        ],
    );
    assert_eq!(
        keys(&problems),
        [
            "admission.headroom_pct",
            "benchmark.quiet_secs",
            "presets.render.min_cores",
        ]
    );
    assert_eq!(problems[0].1, "must be at least 0 and below 100");
    assert_eq!(problems[2].1, "must be at most cores");
}

#[test]
fn test_edit_refuses_changes_that_do_not_fit_their_setting() {
    let problems = invalid(
        "",
        &[
            number("estimates.min_runs", 2.5),
            number("retention.lease_days", -1.0),
            number("daemon.port", 70_000.0),
            switch("admission.headroom_pct", true),
            number("admission.nope", 1.0),
            set_row("classes", "huge", profile(1.0, 1, 0.0, 0, 1), None),
            number("classes", 1.0),
        ],
    );
    let messages: Vec<String> = problems
        .iter()
        .map(|(key, message)| format!("{key} {message}"))
        .collect();
    assert_eq!(
        messages,
        [
            "estimates.min_runs must be a whole number",
            "retention.lease_days must be at least 0",
            "daemon.port is too large",
            "admission.headroom_pct takes a number",
            "admission.nope is not a setting",
            "classes.huge is not a row of this table",
            "classes is a table, changed one row at a time",
        ]
    );
}

#[test]
fn test_edit_refuses_bad_preset_names_and_blank_rules() {
    let problems = invalid(
        "",
        &[
            set_row("presets", "Big", profile(1.0, 1, 0.0, 0, 1), None),
            Change::SetRules {
                rules: vec![rule(" ", &[], JobClass::CpuHeavy)],
            },
        ],
    );
    assert_eq!(
        keys(&problems),
        ["presets.Big", "classifier.rules[0].program"]
    );
}

#[test]
fn test_edit_reports_a_file_it_cannot_change() {
    assert!(matches!(
        config_write::edit("[admission\n", &[]),
        Err(WriteError::File(_))
    ));
    assert!(matches!(
        config_write::edit(
            "[admission]\ntypo = 1\n",
            &[number("admission.headroom_pct", 5.0)]
        ),
        Err(WriteError::File(_))
    ));
    let problems = invalid("estimates = 3\n", &[number("estimates.min_runs", 4.0)]);
    assert_eq!(keys(&problems), ["estimates.min_runs"]);
}

const START: &str = "# mine\n[admission]\nheadroom_pct = 20.0  # gaming\n";

fn config_file(text: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    fs::write(&path, text).unwrap();
    (dir, path)
}

/// Files in `dir` other than `config.toml`, such as a temporary file left behind.
fn leftovers(dir: &Path) -> Vec<String> {
    fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != "config.toml")
        .collect()
}

#[test]
fn test_apply_replaces_file_and_returns_its_revision() {
    let (dir, path) = config_file(START);
    let revision = config_write::read(&path).unwrap().revision;

    let applied =
        config_write::apply(&path, revision, &[number("admission.headroom_pct", 30.0)]).unwrap();

    let now = config_write::read(&path).unwrap();
    assert_eq!(
        now.text,
        "# mine\n[admission]\nheadroom_pct = 30.0  # gaming\n"
    );
    assert_eq!(applied.revision, now.revision);
    assert_eq!(applied.config.admission.headroom_pct, 30.0);
    assert!(applied.written);
    assert!(leftovers(dir.path()).is_empty());
}

#[test]
fn test_apply_unchanged_value_leaves_file_alone() {
    let (_dir, path) = config_file(START);
    let revision = config_write::read(&path).unwrap().revision;
    let applied =
        config_write::apply(&path, revision, &[number("admission.headroom_pct", 20.0)]).unwrap();
    assert!(!applied.written);
    assert_eq!(applied.revision, revision);
}

#[test]
fn test_apply_invalid_change_leaves_file_untouched() {
    let (dir, path) = config_file(START);
    let revision = config_write::read(&path).unwrap().revision;
    let result = config_write::apply(
        &path,
        revision,
        &[
            number("admission.headroom_pct", 30.0),
            number("estimates.margin_pct", 101.0),
        ],
    );
    match result {
        Err(WriteError::Invalid(problems)) => {
            assert_eq!(problems.len(), 1);
            assert_eq!(problems[0].key, "estimates.margin_pct");
        }
        other => panic!("expected invalid changes, got {other:?}"),
    }
    assert_eq!(fs::read_to_string(&path).unwrap(), START);
    assert!(leftovers(dir.path()).is_empty());
}

#[test]
fn test_apply_after_hand_edit_conflicts_and_leaves_file() {
    let (dir, path) = config_file(START);
    let revision = config_write::read(&path).unwrap().revision;
    let hand = format!("{START}\n[estimates]\nlower = false\n");
    fs::write(&path, &hand).unwrap();

    let change = [number("admission.headroom_pct", 30.0)];
    match config_write::apply(&path, revision, &change) {
        Err(WriteError::Conflict { current }) => {
            assert_eq!(current, Revision::of(hand.as_bytes()));
        }
        other => panic!("expected a conflict, got {other:?}"),
    }
    assert_eq!(fs::read_to_string(&path).unwrap(), hand);
    assert!(leftovers(dir.path()).is_empty());

    // The same change applies again on top of the hand edit.
    let revision = config_write::read(&path).unwrap().revision;
    config_write::apply(&path, revision, &change).unwrap();
    let config = Config::load(&path).unwrap();
    assert_eq!(config.admission.headroom_pct, 30.0);
    assert!(!config.estimates.lower);
}

#[test]
fn test_apply_missing_file_creates_it() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    let snapshot = config_write::read(&path).unwrap();
    assert_eq!(snapshot.text, "");
    config_write::apply(
        &path,
        snapshot.revision,
        &[number("admission.headroom_pct", 25.0)],
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "[admission]\nheadroom_pct = 25.0\n"
    );
}

#[cfg(unix)]
#[test]
fn test_apply_keeps_file_mode() {
    use std::os::unix::fs::PermissionsExt;

    let (_dir, path) = config_file(START);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let revision = config_write::read(&path).unwrap().revision;
    config_write::apply(&path, revision, &[number("admission.headroom_pct", 30.0)]).unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[cfg(unix)]
#[test]
fn test_apply_writes_through_symlink() {
    let dir = TempDir::new().unwrap();
    let real = dir.path().join("dotfiles").join("allot.toml");
    fs::create_dir(real.parent().unwrap()).unwrap();
    fs::write(&real, START).unwrap();
    let link = dir.path().join("config.toml");
    std::os::unix::fs::symlink("dotfiles/allot.toml", &link).unwrap();

    let revision = config_write::read(&link).unwrap().revision;
    config_write::apply(&link, revision, &[number("admission.headroom_pct", 30.0)]).unwrap();

    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        fs::read_to_string(&real)
            .unwrap()
            .contains("headroom_pct = 30.0")
    );
}

/// Windows: replacing the file while another handle holds it open, the way an editor or a
/// scanner might. The handle lives in this test's own process, so nothing else is touched.
#[cfg(windows)]
mod held {
    use std::{
        fs::{self, File, OpenOptions},
        os::windows::fs::OpenOptionsExt,
        path::Path,
        thread,
        time::{Duration, Instant},
    };

    use super::{START, config_file, leftovers, number};
    use allot_shared::config_write::{self, WriteError};

    const SHARE_READ: u32 = 1;
    const SHARE_WRITE: u32 = 2;
    const SHARE_DELETE: u32 = 4;

    fn hold(path: &Path, share: u32) -> File {
        OpenOptions::new()
            .read(true)
            .share_mode(share)
            .open(path)
            .unwrap()
    }

    /// One write per share mode. Only a holder that allows delete lets the replace through;
    /// every other holder blocks it until the retries run out, and one that shares nothing
    /// even blocks reading the file.
    #[test]
    fn test_apply_by_share_mode() {
        let cases = [
            ("none", 0, false),
            ("read", SHARE_READ, false),
            ("read+write", SHARE_READ | SHARE_WRITE, false),
            (
                "read+write+delete",
                SHARE_READ | SHARE_WRITE | SHARE_DELETE,
                true,
            ),
        ];
        for (name, share, replaced) in cases {
            let (dir, path) = config_file(START);
            let revision = config_write::read(&path).unwrap().revision;
            let holder = hold(&path, share);
            let result =
                config_write::apply(&path, revision, &[number("admission.headroom_pct", 30.0)]);
            drop(holder);

            match (&result, replaced) {
                (Ok(_), true) => {
                    assert!(
                        fs::read_to_string(&path).unwrap().contains("30.0"),
                        "{name}"
                    );
                }
                (Err(WriteError::Held(_)), false) => {
                    assert_eq!(fs::read_to_string(&path).unwrap(), START, "{name}");
                }
                _ => panic!("{name}: unexpected {result:?}"),
            }
            assert!(
                leftovers(dir.path()).is_empty(),
                "{name}: temporary file left"
            );
        }
    }

    #[test]
    fn test_apply_held_error_tells_user_what_to_do() {
        let (_dir, path) = config_file(START);
        let revision = config_write::read(&path).unwrap().revision;
        let _holder = hold(&path, SHARE_READ);
        let started = Instant::now();
        let error = config_write::apply(&path, revision, &[number("admission.headroom_pct", 30.0)])
            .unwrap_err();
        assert!(
            started.elapsed() >= Duration::from_millis(900),
            "gave up too early"
        );
        assert_eq!(
            error.to_string(),
            "config.toml is open in another program that blocks changes. Close it there and \
             try again."
        );
    }

    /// A holder that lets go within the retry budget only delays the write.
    #[test]
    fn test_apply_retry_outlasts_short_hold() {
        let (_dir, path) = config_file(START);
        let revision = config_write::read(&path).unwrap().revision;
        let holder = hold(&path, SHARE_READ | SHARE_WRITE);
        let release = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            drop(holder);
        });
        let applied =
            config_write::apply(&path, revision, &[number("admission.headroom_pct", 30.0)]);
        release.join().unwrap();
        assert!(applied.unwrap().written);
        assert!(fs::read_to_string(&path).unwrap().contains("30.0"));
    }
}
