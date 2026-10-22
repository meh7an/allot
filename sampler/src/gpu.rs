//! Parsing of PDH GPU instance names and grouping of engine load the way Task Manager does.

use std::collections::HashMap;

use rh_core::sampler::EngineLoad;

/// Engine groups the snapshot reports. Engines of other types are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EngineKind {
    ThreeD,
    Compute,
    Copy,
    Video,
}

impl EngineKind {
    fn from_engtype(engtype: &str) -> Option<Self> {
        let engtype = engtype.to_ascii_lowercase();
        if engtype.starts_with("3d") {
            Some(Self::ThreeD)
        } else if engtype.starts_with("compute") || engtype.starts_with("cuda") {
            Some(Self::Compute)
        } else if engtype.starts_with("copy") {
            Some(Self::Copy)
        } else if engtype.starts_with("video") {
            Some(Self::Video)
        } else {
            None
        }
    }
}

/// Formats an adapter LUID the way PDH instance names spell it, in lowercase.
pub(crate) fn format_luid(high: u32, low: u32) -> String {
    format!("0x{high:08x}_0x{low:08x}")
}

/// Instance of `\GPU Engine(*)`, for example
/// `pid_1234_luid_0x00000000_0x0000D1A2_phys_0_eng_3_engtype_VideoDecode`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EngineInstance {
    pid: u32,
    luid: String,
    engine: String,
    kind: Option<EngineKind>,
}

fn parse_engine(name: &str) -> Option<EngineInstance> {
    let tokens: Vec<&str> = name.split('_').collect();
    let engtype_at = tokens.iter().position(|&token| token == "engtype")?;
    Some(EngineInstance {
        pid: field(&tokens, "pid")?.parse().ok()?,
        luid: parse_luid(&tokens)?,
        engine: format!("{}_{}", field(&tokens, "phys")?, field(&tokens, "eng")?),
        kind: EngineKind::from_engtype(&tokens[engtype_at + 1..].join("_")),
    })
}

/// Parses an instance of `\GPU Process Memory(*)`, for example
/// `pid_1234_luid_0x00000000_0x0000D1A2_phys_0`, into its PID and LUID.
pub(crate) fn parse_process_memory(name: &str) -> Option<(u32, String)> {
    let tokens: Vec<&str> = name.split('_').collect();
    Some((field(&tokens, "pid")?.parse().ok()?, parse_luid(&tokens)?))
}

/// Parses an instance of `\GPU Adapter Memory(*)`, for example
/// `luid_0x00000000_0x0000D1A2_phys_0`, into its LUID.
pub(crate) fn parse_adapter_memory(name: &str) -> Option<String> {
    let tokens: Vec<&str> = name.split('_').collect();
    parse_luid(&tokens)
}

fn field<'a>(tokens: &[&'a str], key: &str) -> Option<&'a str> {
    let at = tokens.iter().position(|&token| token == key)?;
    tokens.get(at + 1).copied()
}

fn parse_luid(tokens: &[&str]) -> Option<String> {
    let at = tokens.iter().position(|&token| token == "luid")?;
    let high = u32::from_str_radix(tokens.get(at + 1)?.strip_prefix("0x")?, 16).ok()?;
    let low = u32::from_str_radix(tokens.get(at + 2)?.strip_prefix("0x")?, 16).ok()?;
    Some(format_luid(high, low))
}

/// Engine load per adapter and GPU load per process.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct GpuLoad {
    /// Keyed by LUID.
    pub(crate) adapters: HashMap<String, EngineLoad>,
    /// Keyed by PID: the load of the process's busiest engine.
    pub(crate) processes: HashMap<u32, f32>,
}

/// Groups `\GPU Engine(*)\Utilization Percentage` readings like Task Manager: an engine's
/// load is the sum over the processes using it, an engine group shows its busiest engine,
/// and a process shows its busiest engine.
pub(crate) fn group_engine_load(readings: &[(String, f64)]) -> GpuLoad {
    let mut engines: HashMap<(String, String), (Option<EngineKind>, f32)> = HashMap::new();
    let mut load = GpuLoad::default();
    for (name, value) in readings {
        let Some(instance) = parse_engine(name) else {
            continue;
        };
        let value = *value as f32;
        let process = load.processes.entry(instance.pid).or_default();
        *process = process.max(value);
        let engine = engines
            .entry((instance.luid, instance.engine))
            .or_insert((instance.kind, 0.0));
        engine.1 += value;
    }
    for ((luid, _), (kind, value)) in engines {
        let adapter = load.adapters.entry(luid).or_default();
        let group = match kind {
            Some(EngineKind::ThreeD) => &mut adapter.three_d_pct,
            Some(EngineKind::Compute) => &mut adapter.compute_pct,
            Some(EngineKind::Copy) => &mut adapter.copy_pct,
            Some(EngineKind::Video) => &mut adapter.video_pct,
            None => continue,
        };
        *group = group.max(value.min(100.0));
    }
    load
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUID: &str = "0x00000000_0x0000d1a2";

    fn reading(name: &str, value: f64) -> (String, f64) {
        (name.into(), value)
    }

    #[test]
    fn test_format_luid_pads_and_lowercases() {
        assert_eq!(format_luid(0, 0xD1A2), LUID);
        assert_eq!(format_luid(1, 0xABCDEF01), "0x00000001_0xabcdef01");
    }

    #[test]
    fn test_parse_engine_reads_every_part() {
        let instance =
            parse_engine("pid_1234_luid_0x00000000_0x0000D1A2_phys_0_eng_3_engtype_VideoDecode");
        assert_eq!(
            instance,
            Some(EngineInstance {
                pid: 1234,
                luid: LUID.into(),
                engine: "0_3".into(),
                kind: Some(EngineKind::Video),
            })
        );
    }

    #[test]
    fn test_parse_engine_engtype_with_underscore_and_unknown_type() {
        let compute = parse_engine("pid_1_luid_0x0_0x1_phys_0_eng_5_engtype_Compute_0").unwrap();
        assert_eq!(compute.kind, Some(EngineKind::Compute));
        let other = parse_engine("pid_1_luid_0x0_0x1_phys_0_eng_9_engtype_Security").unwrap();
        assert_eq!(other.kind, None);
        let unnamed = parse_engine("pid_1_luid_0x0_0x1_phys_0_eng_9_engtype_").unwrap();
        assert_eq!(unnamed.kind, None);
    }

    #[test]
    fn test_parse_malformed_names_are_none() {
        assert_eq!(parse_engine("_Total"), None);
        assert_eq!(
            parse_engine("pid_x_luid_0x0_0x1_phys_0_eng_0_engtype_3D"),
            None
        );
        assert_eq!(parse_process_memory("pid_12_luid_zz_0x1_phys_0"), None);
        assert_eq!(parse_adapter_memory("phys_0"), None);
    }

    #[test]
    fn test_parse_memory_instances_read_pid_and_luid() {
        assert_eq!(
            parse_process_memory("pid_42_luid_0x00000000_0x0000D1A2_phys_0"),
            Some((42, LUID.into()))
        );
        assert_eq!(
            parse_adapter_memory("luid_0x00000000_0x0000D1A2_phys_0"),
            Some(LUID.into())
        );
    }

    #[test]
    fn test_group_engine_load_sums_per_engine_and_takes_busiest_per_group() {
        let load = group_engine_load(&[
            reading("pid_1_luid_0x0_0xd1a2_phys_0_eng_0_engtype_3D", 30.0),
            reading("pid_2_luid_0x0_0xd1a2_phys_0_eng_0_engtype_3D", 25.0),
            reading("pid_2_luid_0x0_0xd1a2_phys_0_eng_1_engtype_3D", 40.0),
            reading("pid_3_luid_0x0_0xd1a2_phys_0_eng_4_engtype_Copy", 5.0),
            reading("pid_3_luid_0x0_0xd1a2_phys_0_eng_7_engtype_Security", 99.0),
            reading("pid_4_luid_0x0_0x0001_phys_0_eng_0_engtype_Compute_0", 70.0),
            reading("_Total", 100.0),
        ]);

        let main = load.adapters[LUID];
        assert_eq!(main.three_d_pct, 55.0);
        assert_eq!(main.copy_pct, 5.0);
        assert_eq!(main.compute_pct, 0.0);
        assert_eq!(load.adapters["0x00000000_0x00000001"].compute_pct, 70.0);
        assert_eq!(load.processes[&1], 30.0);
        assert_eq!(load.processes[&2], 40.0);
        assert_eq!(load.processes[&3], 99.0);
    }

    #[test]
    fn test_group_engine_load_caps_engine_at_100() {
        let load = group_engine_load(&[
            reading("pid_1_luid_0x0_0x1_phys_0_eng_0_engtype_3D", 70.0),
            reading("pid_2_luid_0x0_0x1_phys_0_eng_0_engtype_3D", 60.0),
        ]);
        assert_eq!(load.adapters["0x00000000_0x00000001"].three_d_pct, 100.0);
    }
}
