//! Round-trip fixtures: every edit leaves the rest of the file byte-identical, and the file
//! then reads as the old config with exactly the intended change.
//!
//! Expected outputs live in `tests/expected/`. `ALLOT_BLESS=1` rewrites them; review the
//! diff against the fixture before keeping a blessed file.

use std::{env, fs, path::PathBuf};

use allot_shared::{
    config::{Config, HeavyRule, Presets, Sizing},
    types::{JobClass, JobProfile},
};
use config_writeback::{edit, rules};
use toml_edit::{DocumentMut, Value};

struct Case {
    name: &'static str,
    fixture: &'static str,
    edit: fn(&mut DocumentMut),
    expect: fn(&mut Config),
}

fn dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
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

const HAND: &str = "hand_edited.toml";
const INLINE: &str = "inline_rules.toml";

const CASES: &[Case] = &[
    Case {
        name: "scalar_change",
        fixture: HAND,
        edit: |doc| edit::set(doc, &["admission", "headroom_pct"], 30.0),
        expect: |config| config.admission.headroom_pct = 30.0,
    },
    Case {
        name: "scalar_bool_change",
        fixture: HAND,
        edit: |doc| edit::set(doc, &["estimates", "lower"], true),
        expect: |config| config.estimates.lower = true,
    },
    Case {
        name: "scalar_new_key",
        fixture: HAND,
        edit: |doc| edit::set(doc, &["estimates", "min_runs"], 5),
        expect: |config| config.estimates.min_runs = 5,
    },
    Case {
        name: "scalar_new_table",
        fixture: HAND,
        edit: |doc| edit::set(doc, &["benchmark", "quiet_secs"], 20),
        expect: |config| config.benchmark.quiet_secs = 20,
    },
    Case {
        name: "scalar_remove_empties_table",
        fixture: HAND,
        edit: |doc| assert!(edit::remove(doc, &["estimates", "lower"])),
        expect: |config| config.estimates.lower = true,
    },
    Case {
        name: "scalar_remove_keeps_commented_table",
        fixture: HAND,
        edit: |doc| assert!(edit::remove(doc, &["daemon", "port"])),
        expect: |_| {},
    },
    Case {
        name: "preset_new_table",
        fixture: HAND,
        edit: |doc| {
            let entries = [
                ("cores", Value::from(12.0)),
                ("ram_mb", Value::from(16_384)),
                ("min_cores", Value::from(4.0)),
            ];
            edit::set_table(doc, &["presets", "big"], &entries);
        },
        expect: |config| {
            let profile = JobProfile {
                cores: 12.0,
                ram_mb: 16_384,
                gpu_pct: 0.0,
                vram_mb: 0,
                est_secs: 300,
            };
            let sizing = Sizing {
                profile,
                min_cores: Some(4.0),
            };
            config.presets.0.insert("big".into(), sizing);
        },
    },
    Case {
        name: "preset_inline_min_cores",
        fixture: HAND,
        edit: |doc| edit::set(doc, &["presets", "quick", "min_cores"], 1.5),
        expect: |config| config.presets.0.get_mut("quick").unwrap().min_cores = Some(1.5),
    },
    Case {
        name: "preset_whole_table",
        fixture: HAND,
        edit: |doc| {
            let entries = [
                ("cores", Value::from(6.0)),
                ("ram_mb", Value::from(8_192)),
                ("gpu_pct", Value::from(60.0)),
                ("vram_mb", Value::from(6_144)),
                ("min_cores", Value::from(4.0)),
            ];
            edit::set_table(doc, &["presets", "render"], &entries);
        },
        expect: |config| {
            let render = config.presets.0.get_mut("render").unwrap();
            render.min_cores = Some(4.0);
            render.profile.est_secs = 300;
        },
    },
    Case {
        name: "preset_remove_table",
        fixture: HAND,
        edit: |doc| assert!(edit::remove(doc, &["presets", "cpu_small"])),
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
        edit: |doc| assert!(edit::remove(doc, &["presets", "quick"])),
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
        edit: |doc| rules::set(doc, &reordered_hand_rules()),
        expect: |config| config.classifier.rules = reordered_hand_rules().to_vec(),
    },
    Case {
        name: "rules_append",
        fixture: HAND,
        edit: |doc| rules::set(doc, &appended_hand_rules()),
        expect: |config| config.classifier.rules = appended_hand_rules().to_vec(),
    },
    Case {
        name: "rules_empty",
        fixture: HAND,
        edit: |doc| rules::set(doc, &[]),
        expect: |config| config.classifier.rules.clear(),
    },
    Case {
        name: "rules_reset",
        fixture: HAND,
        edit: |doc| assert!(rules::reset(doc)),
        expect: |config| config.classifier.rules = Config::default().classifier.rules,
    },
    Case {
        name: "dotted_change",
        fixture: INLINE,
        edit: |doc| edit::set(doc, &["admission", "headroom_pct"], 10.0),
        expect: |config| config.admission.headroom_pct = 10.0,
    },
    Case {
        name: "dotted_new_key",
        fixture: INLINE,
        edit: |doc| edit::set(doc, &["estimates", "margin_pct"], 5.0),
        expect: |config| config.estimates.margin_pct = 5.0,
    },
    Case {
        name: "inline_class_min_cores",
        fixture: INLINE,
        edit: |doc| edit::set(doc, &["classes", "cpu_heavy", "min_cores"], 5.0),
        expect: |config| config.classes.cpu_heavy.min_cores = Some(5.0),
    },
    Case {
        name: "inline_new_table",
        fixture: INLINE,
        edit: |doc| edit::set(doc, &["hook", "fail_open"], false),
        expect: |config| config.hook.fail_open = false,
    },
    Case {
        name: "inline_rules_rewrite",
        fixture: INLINE,
        edit: |doc| rules::set(doc, &rewritten_inline_rules()),
        expect: |config| config.classifier.rules = rewritten_inline_rules().to_vec(),
    },
    Case {
        name: "inline_rules_empty",
        fixture: INLINE,
        edit: |doc| rules::set(doc, &[]),
        expect: |config| config.classifier.rules.clear(),
    },
];

/// pnpm and cargo swapped, blender dropped, msbuild added between them.
fn reordered_hand_rules() -> [HeavyRule; 3] {
    let [cargo, pnpm, _] = hand_edited_rules();
    [pnpm, rule("msbuild", &[], JobClass::CpuHeavy), cargo]
}

fn appended_hand_rules() -> [HeavyRule; 4] {
    let [cargo, pnpm, blender] = hand_edited_rules();
    [cargo, pnpm, blender, rule("ninja", &[], JobClass::CpuHeavy)]
}

/// make dropped, ninja added at the end.
fn rewritten_inline_rules() -> [HeavyRule; 3] {
    let [cargo, _, python] = inline_rules();
    [cargo, python, rule("ninja", &["-j*"], JobClass::CpuHeavy)]
}

fn apply(case: &Case, input: &str) -> String {
    let mut doc: DocumentMut = input.parse().unwrap();
    (case.edit)(&mut doc);
    doc.to_string()
}

#[test]
fn test_fixtures_hold_their_documented_rules() {
    let hand = Config::from_toml(&fixture(HAND)).unwrap();
    assert_eq!(hand.classifier.rules, hand_edited_rules());
    let inline = Config::from_toml(&fixture(INLINE)).unwrap();
    assert_eq!(inline.classifier.rules, inline_rules());
}

#[test]
fn test_fixtures_parse_and_print_byte_identical() {
    for name in [HAND, INLINE] {
        let text = fixture(name);
        let doc: DocumentMut = text.parse().unwrap();
        assert_eq!(doc.to_string(), text, "{name}");
    }
}

/// Every case again on the fixtures saved by Notepad: CRLF endings and a byte order mark.
#[test]
fn test_edits_keep_windows_layout() {
    for case in CASES {
        let windows = |text: &str| format!("\u{feff}{}", text.replace('\n', "\r\n"));
        let input = windows(&fixture(case.fixture));
        let mut doc: DocumentMut = input.parse().unwrap();
        (case.edit)(&mut doc);
        let output = edit::Layout::of(&input).render(&doc);
        let expected = fs::read_to_string(dir("expected").join(format!("{}.toml", case.name)));
        assert_eq!(output, windows(&expected.unwrap()), "{}", case.name);
    }
}

#[test]
fn test_edits_match_expected_files_and_config() {
    let bless = env::var_os("ALLOT_BLESS").is_some();
    let mut failures = Vec::new();
    for case in CASES {
        let input = fixture(case.fixture);
        let output = apply(case, &input);

        let mut wanted = Config::from_toml(&input).unwrap();
        (case.expect)(&mut wanted);
        match Config::from_toml(&output) {
            Ok(read) if read == wanted => {}
            Ok(read) => failures.push(format!("{}: config differs: {read:#?}", case.name)),
            Err(error) => failures.push(format!("{}: output rejected: {error}", case.name)),
        }

        let expected_path = dir("expected").join(format!("{}.toml", case.name));
        if bless {
            fs::create_dir_all(dir("expected")).unwrap();
            fs::write(&expected_path, &output).unwrap();
        } else {
            match fs::read_to_string(&expected_path) {
                Ok(expected) if expected == output => {}
                Ok(_) => failures.push(format!("{}: bytes differ:\n{output}", case.name)),
                Err(error) => failures.push(format!("{}: no expected file: {error}", case.name)),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
