//! The numbers behind the Settings window's live explanations, worked out for values that are
//! proposed but not saved: how many jobs of each size fit on this machine, and how the hook
//! treats a command line.
//!
//! Both take a whole proposed [`Config`] and refuse one that [`Config::problems`] rejects, so
//! they never explain values allotd would not accept. The results are the wire types of
//! [`allot_shared::explain`], which allotd sends as they are.

pub use allot_shared::explain::{
    ClassSize, CommandKind, CommandResult, CommandTest, FitPreview, Fits, KeptFree, SizeFit,
};
use allot_shared::{
    config::{Config, FieldError, Sizing},
    types::{JobClass, JobProfile, Snapshot},
};

use crate::{
    admission::{self, Room},
    classifier::{Classification, Classifier, CommandMatch, Shell},
    ledger::Ledger,
};

/// Counts stop here, so a size that asks for nothing does not count forever.
pub const FIT_LIMIT: u32 = 99;

/// Proposed settings that [`Config::problems`] rejects.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid settings: {}", join(.problems))]
pub struct Invalid {
    /// Every rejected value, in file order.
    pub problems: Vec<FieldError>,
}

/// How many jobs of each class and preset fit now and with every lease ended, if `proposed`
/// were saved: its headroom, sizes and minimums against the ledger's active leases and the
/// load in `snapshot`.
///
/// Each count starts jobs one after another with the ledger's shrink rule
/// ([`Room::fit`]). Queued leases and benchmarks do not hold the count back.
///
/// # Errors
///
/// [`Invalid`] when `proposed` holds a value allotd rejects.
pub fn fit_preview(
    proposed: &Config,
    ledger: &Ledger,
    snapshot: &Snapshot,
) -> Result<FitPreview, Invalid> {
    check(proposed)?;
    let headroom_pct = proposed.admission.headroom_pct;
    let now = ledger.room_with(snapshot, headroom_pct);
    let idle = ledger.ceiling_with(snapshot, headroom_pct);
    let size_fit = |name: &str, sizing: &Sizing| SizeFit {
        name: name.to_owned(),
        min_cores: start_minimum(sizing),
        now: count(now, sizing),
        idle: count(idle, sizing),
    };
    Ok(FitPreview {
        classes: JobClass::ALL
            .iter()
            .map(|&class| size_fit(class.as_str(), proposed.classes.get(class)))
            .collect(),
        presets: proposed
            .presets
            .0
            .iter()
            .map(|(name, sizing)| size_fit(name, sizing))
            .collect(),
        kept_free: kept_free(snapshot, headroom_pct),
        cpu_busy_pct: snapshot.cpu.usage_pct.avg_10s,
    })
}

/// Every simple command of `line`, written for `shell`, as `proposed`'s rules and light flags
/// treat it, with the sizes of its classes.
///
/// # Errors
///
/// [`Invalid`] when `proposed` holds a value allotd rejects.
pub fn test_command(proposed: &Config, line: &str, shell: Shell) -> Result<CommandTest, Invalid> {
    check(proposed)?;
    let classifier = Classifier::new(&proposed.classifier);
    let size = |class: JobClass| {
        let sizing = proposed.classes.get(class);
        ClassSize {
            class,
            profile: sizing.profile,
            min_cores: start_minimum(sizing),
        }
    };
    let commands = classifier
        .explain(line, shell)
        .into_iter()
        .map(|simple| {
            let unmatched = CommandResult {
                command: simple.command,
                kind: CommandKind::NotHeavy,
                rule_index: None,
                flag: None,
                size: None,
            };
            match simple.matched {
                CommandMatch::Heavy { rule_index } => CommandResult {
                    kind: CommandKind::Heavy,
                    // The rule list's length is bounded by the file, far below u32::MAX.
                    rule_index: u32::try_from(rule_index).ok(),
                    size: Some(size(proposed.classifier.rules[rule_index].class)),
                    ..unmatched
                },
                CommandMatch::PrintOnly { flag } => CommandResult {
                    kind: CommandKind::PrintOnly,
                    flag: Some(flag),
                    ..unmatched
                },
                _ => unmatched,
            }
        })
        .collect();
    let lease = match classifier.classify(line, shell) {
        Classification::Heavy { class, .. } => Some(size(class)),
        _ => None,
    };
    Ok(CommandTest { lease, commands })
}

fn check(proposed: &Config) -> Result<(), Invalid> {
    let problems = proposed.problems();
    if problems.is_empty() {
        Ok(())
    } else {
        Err(Invalid { problems })
    }
}

fn join(problems: &[FieldError]) -> String {
    let texts: Vec<String> = problems.iter().map(ToString::to_string).collect();
    texts.join("; ")
}

/// The fewest cores a job of `sizing` starts with; [`Room::fit`] never goes below it.
fn start_minimum(sizing: &Sizing) -> f32 {
    sizing.minimum().min(sizing.profile.cores)
}

/// Starts jobs of `sizing` in `room` until one no longer fits. A shrunk start holds only its
/// minimum cores, so the count says how many could start shrunk, not what one arrival takes.
fn count(mut room: Room, sizing: &Sizing) -> Fits {
    let profile = &sizing.profile;
    let minimum = start_minimum(sizing);
    let at_minimum = JobProfile {
        cores: minimum,
        ..*profile
    };
    let mut fits = Fits::default();
    while let Ok(granted) = room.fit(profile, minimum) {
        if fits.full + fits.shrunk == FIT_LIMIT {
            fits.more = true;
            break;
        }
        if granted.cores < profile.cores {
            fits.shrunk += 1;
            room.reserve(&at_minimum);
        } else {
            fits.full += 1;
            room.reserve(profile);
        }
    }
    fits
}

fn kept_free(snapshot: &Snapshot, headroom_pct: f32) -> KeptFree {
    let share = f64::from(headroom_pct) / 100.0;
    let total_cores = snapshot.cpu.logical_cores;
    let total_ram_mb = snapshot.memory.total_mb;
    let gpu = admission::primary_adapter(&snapshot.gpus);
    let total_vram_mb = gpu
        .filter(|gpu| !gpu.unified_memory)
        .and_then(|gpu| gpu.dedicated_total_mb);
    KeptFree {
        cores: f64::from(total_cores) * share,
        total_cores,
        ram_mb: total_ram_mb as f64 * share,
        total_ram_mb,
        gpu_pct: gpu.map(|_| f64::from(headroom_pct)),
        vram_mb: total_vram_mb.map(|total_mb| total_mb as f64 * share),
        total_vram_mb,
    }
}

#[cfg(test)]
mod tests {
    use allot_shared::config::HeavyRule;

    use super::*;
    use crate::{
        benchmark::QuietLimits,
        ledger::LedgerSettings,
        test_support::{active_lease, gpu, profile, snapshot, unified_gpu},
    };

    /// A ledger whose saved headroom is 10 %, holding `active` leases.
    fn ledger(active: &[(i64, JobProfile)]) -> Ledger {
        let mut ledger = Ledger::new(LedgerSettings {
            headroom_pct: 10.0,
            heartbeat_ttl_ms: 60_000,
            quiet: QuietLimits {
                cpu_pct: 10.0,
                gpu_pct: 10.0,
                window_ms: 10_000,
                max_wait_ms: 60_000,
            },
        });
        for (id, profile) in active {
            let lease = active_lease(*id, &format!("s{id}"), *profile);
            assert!(ledger.restore(lease, 0).is_ok());
        }
        ledger
    }

    fn with_headroom(headroom_pct: f32) -> Config {
        let mut config = Config::default();
        config.admission.headroom_pct = headroom_pct;
        config
    }

    fn fits(full: u32, shrunk: u32) -> Fits {
        Fits {
            full,
            shrunk,
            more: false,
        }
    }

    fn class_fit(preview: &FitPreview, class: JobClass) -> &SizeFit {
        let found = preview
            .classes
            .iter()
            .find(|fit| fit.name == class.as_str());
        found.unwrap()
    }

    fn preset_fit<'a>(preview: &'a FitPreview, name: &str) -> &'a SizeFit {
        preview.presets.iter().find(|fit| fit.name == name).unwrap()
    }

    fn rule(program: &str, args: &[&str], class: JobClass) -> HeavyRule {
        HeavyRule {
            program: program.into(),
            args: args.iter().map(|&arg| arg.into()).collect(),
            class,
        }
    }

    #[test]
    fn test_fit_preview_idle_machine_counts_full_then_shrunk_at_minimum() {
        // 12 of 16 cores usable: one 8-core job, then 4 cores, exactly its minimum.
        let preview = fit_preview(&with_headroom(25.0), &ledger(&[]), &snapshot(0.0, 0, None));
        let cpu_heavy = class_fit(&preview.unwrap(), JobClass::CpuHeavy).clone();
        assert_eq!(cpu_heavy.name, "cpu_heavy");
        assert_eq!(cpu_heavy.min_cores, 4.0);
        assert_eq!(cpu_heavy.now, fits(1, 1));
        assert_eq!(cpu_heavy.idle, fits(1, 1));
    }

    #[test]
    fn test_fit_preview_packs_shrunk_jobs_at_their_minimum() {
        let mut config = with_headroom(0.0);
        config.classes.cpu_heavy.min_cores = Some(2.0);
        let ledger = ledger(&[]);
        // 16 cores: two 8-core jobs, and nothing left; at 12.5 % headroom, 14 cores: one
        // full job, then three more at 2 cores.
        let full = fit_preview(&config, &ledger, &snapshot(0.0, 0, None)).unwrap();
        assert_eq!(class_fit(&full, JobClass::CpuHeavy).now, fits(2, 0));
        config.admission.headroom_pct = 12.5;
        let packed = fit_preview(&config, &ledger, &snapshot(0.0, 0, None)).unwrap();
        assert_eq!(class_fit(&packed, JobClass::CpuHeavy).now, fits(1, 3));
    }

    #[test]
    fn test_fit_preview_uses_proposed_headroom_not_the_saved_one() {
        // The ledger keeps 10 %; the proposal keeps half the machine, 8 cores.
        let preview = fit_preview(&with_headroom(50.0), &ledger(&[]), &snapshot(0.0, 0, None));
        assert_eq!(
            class_fit(&preview.unwrap(), JobClass::CpuHeavy).now,
            fits(1, 0)
        );
    }

    #[test]
    fn test_fit_preview_active_leases_count_now_but_not_when_idle() {
        let ledger = ledger(&[(1, profile(8.0, 4_096))]);
        let preview = fit_preview(&with_headroom(0.0), &ledger, &snapshot(0.0, 0, None)).unwrap();
        let cpu_heavy = class_fit(&preview, JobClass::CpuHeavy);
        assert_eq!(cpu_heavy.now, fits(1, 0));
        assert_eq!(cpu_heavy.idle, fits(2, 0));
    }

    #[test]
    fn test_fit_preview_background_load_stays_when_idle() {
        // Half of 16 cores busy outside leases leaves 8 now and with every lease ended.
        let preview =
            fit_preview(&with_headroom(0.0), &ledger(&[]), &snapshot(50.0, 0, None)).unwrap();
        let cpu_heavy = class_fit(&preview, JobClass::CpuHeavy);
        assert_eq!(cpu_heavy.now, fits(1, 0));
        assert_eq!(cpu_heavy.idle, fits(1, 0));
        assert_eq!(preview.cpu_busy_pct, 50.0);
    }

    #[test]
    fn test_fit_preview_memory_never_shrinks() {
        let mut config = with_headroom(0.0);
        config
            .presets
            .0
            .insert("big_memory".into(), Sizing::of(profile(1.0, 20_000)));
        // 22,768 MB free fits one 20,000 MB job, and fewer cores would not help a second.
        let preview = fit_preview(&config, &ledger(&[]), &snapshot(0.0, 10_000, None)).unwrap();
        let big = preset_fit(&preview, "big_memory");
        assert_eq!(big.now, fits(1, 0));
        assert_eq!(big.idle, fits(1, 0));
    }

    #[test]
    fn test_fit_preview_gpu_classes_need_a_gpu() {
        let config = with_headroom(0.0);
        let without = fit_preview(&config, &ledger(&[]), &snapshot(0.0, 0, None)).unwrap();
        assert_eq!(class_fit(&without, JobClass::GpuHeavy).now, fits(0, 0));
        let adapter = Some(gpu(0.0, 0, Some(8_192)));
        let with = fit_preview(&config, &ledger(&[]), &snapshot(0.0, 0, adapter)).unwrap();
        // 80 % of the GPU each: one fits.
        assert_eq!(class_fit(&with, JobClass::GpuHeavy).now, fits(1, 0));
    }

    #[test]
    fn test_fit_preview_size_asking_nothing_stops_at_the_limit() {
        let mut config = with_headroom(15.0);
        config
            .presets
            .0
            .insert("nothing".into(), Sizing::of(profile(0.0, 0)));
        let preview = fit_preview(&config, &ledger(&[]), &snapshot(0.0, 0, None)).unwrap();
        let nothing = preset_fit(&preview, "nothing");
        assert_eq!(nothing.now.full, FIT_LIMIT);
        assert!(nothing.now.more);
    }

    #[test]
    fn test_fit_preview_lists_every_class_and_preset() {
        let config = Config::default();
        let preview = fit_preview(&config, &ledger(&[]), &snapshot(0.0, 0, None)).unwrap();
        let classes: Vec<&str> = preview
            .classes
            .iter()
            .map(|fit| fit.name.as_str())
            .collect();
        assert_eq!(classes, ["light", "cpu_heavy", "gpu_heavy", "mixed"]);
        let presets: Vec<&str> = preview
            .presets
            .iter()
            .map(|fit| fit.name.as_str())
            .collect();
        let expected: Vec<&str> = config.presets.names().collect();
        assert_eq!(presets, expected);
    }

    #[test]
    fn test_fit_preview_kept_free_follows_proposed_headroom() {
        let adapter = Some(gpu(0.0, 0, Some(8_192)));
        let preview = fit_preview(
            &with_headroom(15.0),
            &ledger(&[]),
            &snapshot(0.0, 0, adapter),
        )
        .unwrap();
        let kept = preview.kept_free;
        assert!((kept.cores - 2.4).abs() < 1e-6);
        assert_eq!(kept.total_cores, 16);
        assert!((kept.ram_mb - 4_915.2).abs() < 1e-6);
        assert_eq!(kept.total_ram_mb, 32_768);
        assert_eq!(kept.gpu_pct, Some(15.0));
        assert!(
            kept.vram_mb
                .is_some_and(|vram_mb| (vram_mb - 1_228.8).abs() < 1e-6)
        );
        assert_eq!(kept.total_vram_mb, Some(8_192));
    }

    #[test]
    fn test_fit_preview_kept_free_has_no_video_memory_without_dedicated_memory() {
        let none = fit_preview(&with_headroom(15.0), &ledger(&[]), &snapshot(0.0, 0, None));
        let none = none.unwrap().kept_free;
        assert_eq!(
            (none.gpu_pct, none.vram_mb, none.total_vram_mb),
            (None, None, None)
        );
        let unified = Some(unified_gpu(0.0, 0));
        let shared = fit_preview(
            &with_headroom(15.0),
            &ledger(&[]),
            &snapshot(0.0, 0, unified),
        );
        let shared = shared.unwrap().kept_free;
        assert_eq!(shared.gpu_pct, Some(15.0));
        assert_eq!((shared.vram_mb, shared.total_vram_mb), (None, None));
    }

    #[test]
    fn test_fit_preview_invalid_proposal_names_the_key() {
        let mut config = with_headroom(100.0);
        config.classes.mixed.min_cores = Some(0.0);
        let Err(invalid) = fit_preview(&config, &ledger(&[]), &snapshot(0.0, 0, None)) else {
            panic!("an invalid proposal was explained");
        };
        let keys: Vec<&str> = invalid.problems.iter().map(|p| p.key.as_str()).collect();
        assert_eq!(keys, ["admission.headroom_pct", "classes.mixed.min_cores"]);
    }

    #[test]
    fn test_test_command_sizes_come_from_the_proposal() {
        let mut config = Config::default();
        config.classes.cpu_heavy.profile.cores = 6.0;
        let test = test_command(&config, "cargo build --release && cargo fmt", Shell::Bash);
        let test = test.unwrap();
        let heavy = &test.commands[0];
        assert_eq!(heavy.command, "cargo build --release");
        assert_eq!(
            (heavy.kind, heavy.rule_index),
            (CommandKind::Heavy, Some(0))
        );
        let size = heavy.size.unwrap();
        assert_eq!(size.class, JobClass::CpuHeavy);
        assert_eq!(size.profile.cores, 6.0);
        assert_eq!(size.min_cores, 3.0);
        let fmt = &test.commands[1];
        assert_eq!(
            (fmt.command.as_str(), fmt.kind, fmt.rule_index),
            ("cargo fmt", CommandKind::NotHeavy, None)
        );
        assert_eq!(fmt.size, None);
        assert_eq!(test.lease, Some(size));
    }

    #[test]
    fn test_test_command_uses_the_edited_rules() {
        let mut config = Config::default();
        config.classifier.rules = vec![
            rule("python*", &["train.py"], JobClass::GpuHeavy),
            rule("make", &[], JobClass::CpuHeavy),
        ];
        let test = test_command(
            &config,
            "cargo build; python3 train.py; make -j8",
            Shell::Bash,
        );
        let matched: Vec<(CommandKind, Option<u32>)> = test
            .unwrap()
            .commands
            .into_iter()
            .map(|result| (result.kind, result.rule_index))
            .collect();
        assert_eq!(
            matched,
            [
                (CommandKind::NotHeavy, None),
                (CommandKind::Heavy, Some(0)),
                (CommandKind::Heavy, Some(1)),
            ]
        );
    }

    #[test]
    fn test_test_command_lease_combines_every_heavy_command() {
        let mut config = Config::default();
        config.classifier.rules = vec![
            rule("cargo", &["build"], JobClass::CpuHeavy),
            rule("python", &["train.py"], JobClass::GpuHeavy),
        ];
        let test = test_command(&config, "cargo build && python train.py", Shell::Bash).unwrap();
        let lease = test.lease.unwrap();
        assert_eq!(lease.class, JobClass::Mixed);
        assert_eq!(lease.profile, config.classes.mixed.profile);
        assert_eq!(lease.min_cores, config.classes.mixed.minimum());
        let classes: Vec<Option<JobClass>> = test
            .commands
            .iter()
            .map(|result| result.size.map(|size| size.class))
            .collect();
        assert_eq!(
            classes,
            [Some(JobClass::CpuHeavy), Some(JobClass::GpuHeavy)]
        );
    }

    #[test]
    fn test_test_command_print_only_needs_no_lease() {
        let test = test_command(&Config::default(), "cargo build --help", Shell::Bash).unwrap();
        assert_eq!(test.lease, None);
        let command = &test.commands[0];
        assert_eq!(
            (command.kind, command.flag.as_deref()),
            (CommandKind::PrintOnly, Some("--help"))
        );
        assert_eq!(test.commands[0].size, None);
    }

    #[test]
    fn test_test_command_empty_line_has_no_commands() {
        let test = test_command(&Config::default(), "  ", Shell::PowerShell).unwrap();
        assert_eq!(test.lease, None);
        assert!(test.commands.is_empty());
    }

    #[test]
    fn test_test_command_invalid_rule_names_the_key() {
        let mut config = Config::default();
        config.classifier.rules = vec![rule(" ", &[], JobClass::CpuHeavy)];
        let Err(invalid) = test_command(&config, "cargo build", Shell::Bash) else {
            panic!("an invalid rule list was used");
        };
        assert_eq!(invalid.problems[0].key, "classifier.rules[0].program");
    }
}
