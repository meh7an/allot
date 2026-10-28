//! AMD, Intel and Arm GPUs through their DRM drivers: adapters from `/sys/class/drm`, load
//! and memory per process from fdinfo, and amdgpu's whole-device busy percentage and VRAM
//! from sysfs.

use std::{
    collections::HashMap,
    fmt, fs,
    path::{Path, PathBuf},
    time::Instant,
};

use allot_core::sampler::{EngineLoad, RawGpu};
use allot_shared::types::{Accuracy, GpuSource};

use crate::linux::gpu::{
    GpuReader, GpuReading, ProcessGpu, bytes_to_mb,
    fdinfo::{self, Clients, OpenFile},
    pci,
};

/// Drivers that publish engine statistics in fdinfo. The NVIDIA driver and nouveau do not;
/// NVML covers the former.
const DRIVERS: [&str; 7] = ["amdgpu", "i915", "xe", "msm", "panfrost", "panthor", "v3d"];

/// The driver whose sysfs files give whole-device busy percentage and VRAM.
const AMDGPU: &str = "amdgpu";

/// One GPU, found once when the reader starts.
#[derive(Debug, Clone, PartialEq)]
struct Card {
    /// The device directory below the root, for example `sys/class/drm/card0/device`.
    device: PathBuf,
    driver: String,
    /// The device's address, as fdinfo's `drm-pdev` names it.
    address: String,
    name: String,
}

/// Engine groups of the snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EngineGroup {
    ThreeD,
    Compute,
    Copy,
    Video,
}

impl EngineGroup {
    /// The group of an engine as the drivers name them: amdgpu `gfx`, `compute`, `dma`,
    /// `dec`, `enc`; i915 `render`, `copy`, `video`, `video-enhance`, `compute`; xe `rcs`,
    /// `bcs`, `vcs`, `vecs`, `ccs`; msm `gpu`; panfrost `fragment`, `vertex-tiler`; v3d `bin`,
    /// `render`, `tfu`, `csd`.
    fn of(engine: &str) -> Option<Self> {
        const VIDEO: [&str; 8] = ["video", "vcs", "vecs", "dec", "enc", "vcn", "jpeg", "uvd"];
        match engine {
            "gfx" | "render" | "rcs" | "gpu" | "fragment" | "vertex-tiler" | "bin" => {
                Some(Self::ThreeD)
            }
            "compute" | "ccs" | "csd" => Some(Self::Compute),
            "dma" | "copy" | "bcs" | "sdma" | "tfu" => Some(Self::Copy),
            _ if VIDEO.iter().any(|prefix| engine.starts_with(prefix)) => Some(Self::Video),
            _ => None,
        }
    }

    fn of_load(self, load: &mut EngineLoad) -> &mut f32 {
        match self {
            Self::ThreeD => &mut load.three_d_pct,
            Self::Compute => &mut load.compute_pct,
            Self::Copy => &mut load.copy_pct,
            Self::Video => &mut load.video_pct,
        }
    }
}

/// Reads the GPUs whose DRM driver publishes fdinfo statistics.
pub(crate) struct Drm {
    root: PathBuf,
    cards: Vec<Card>,
    /// Finds the open DRM files; fixtures stand in for the fd links of a live machine.
    find_files: fn(&Path) -> Vec<OpenFile>,
    previous: Option<(Instant, Clients)>,
}

impl fmt::Debug for Drm {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Drm")
            .field("root", &self.root)
            .field("cards", &self.cards)
            .finish_non_exhaustive()
    }
}

impl Drm {
    /// The reader for the GPUs below `root`, or `None` when no GPU's driver publishes
    /// fdinfo statistics. Cards are listed once: a GPU added later is not read.
    pub(crate) fn new(root: &Path) -> Option<Self> {
        Self::with_finder(root, fdinfo::drm_files)
    }

    fn with_finder(root: &Path, find_files: fn(&Path) -> Vec<OpenFile>) -> Option<Self> {
        let cards = cards(root);
        (!cards.is_empty()).then(|| Self {
            root: root.to_owned(),
            cards,
            find_files,
            previous: None,
        })
    }

    fn read_number(&self, card: &Card, file: &str) -> Option<u64> {
        let text = fs::read_to_string(self.root.join(&card.device).join(file)).ok()?;
        text.trim().parse().ok()
    }
}

impl GpuReader for Drm {
    fn read(&mut self, now: Instant) -> GpuReading {
        let clients = fdinfo::read_clients(&self.root, &(self.find_files)(&self.root));
        let mut engine_load: HashMap<&str, HashMap<String, f32>> = HashMap::new();
        let mut card_vram: HashMap<&str, u64> = HashMap::new();
        let mut processes: HashMap<u32, ProcessGpu> = HashMap::new();
        let mut process_engines: HashMap<(u32, &str, String), f32> = HashMap::new();
        for (key, client) in &clients {
            let Some(card) = self.cards.iter().find(|card| card.address == key.pdev) else {
                continue;
            };
            // A client shared by several processes is split evenly, so sums stay right.
            let share = client.pids.len().max(1);
            *card_vram.entry(card.address.as_str()).or_default() += client.vram_bytes;
            for &pid in &client.pids {
                processes.entry(pid).or_default().vram_mb +=
                    bytes_to_mb(client.vram_bytes / share as u64);
            }
            let busy = match &self.previous {
                Some((then, previous)) => previous.get(key).map(|before| {
                    let wall_ns = now.saturating_duration_since(*then).as_nanos();
                    client.engine_busy(before, u64::try_from(wall_ns).unwrap_or(u64::MAX))
                }),
                None => None,
            };
            for (engine, pct) in busy.unwrap_or_default() {
                let card_engines = engine_load.entry(card.address.as_str()).or_default();
                *card_engines.entry(engine.clone()).or_default() += pct;
                for &pid in &client.pids {
                    *process_engines
                        .entry((pid, card.address.as_str(), engine.clone()))
                        .or_default() += pct / share as f32;
                }
            }
        }
        // Like Task Manager: a process shows its busiest engine.
        for ((pid, _, _), pct) in process_engines {
            let process = processes.entry(pid).or_default();
            process.gpu_pct = process.gpu_pct.max(pct.min(100.0));
        }

        let adapters = self
            .cards
            .iter()
            .map(|card| {
                let mut engines = EngineLoad::default();
                for (engine, pct) in engine_load.get(card.address.as_str()).into_iter().flatten() {
                    if let Some(group) = EngineGroup::of(engine) {
                        let value = group.of_load(&mut engines);
                        *value = value.max(pct.min(100.0));
                    }
                }
                let amdgpu = card.driver == AMDGPU;
                if amdgpu && let Some(busy) = self.read_number(card, "gpu_busy_percent") {
                    // Counts every user's work, which fdinfo of other users' processes hides.
                    engines.three_d_pct = engines.three_d_pct.max(busy.min(100) as f32);
                }
                let sysfs_vram = amdgpu
                    .then(|| self.read_number(card, "mem_info_vram_used"))
                    .flatten();
                let client_vram = card_vram.get(card.address.as_str()).copied();
                RawGpu {
                    name: card.name.clone(),
                    luid: card.address.clone(),
                    engines,
                    dedicated_used_mb: bytes_to_mb(sysfs_vram.or(client_vram).unwrap_or(0)),
                    dedicated_total_mb: amdgpu
                        .then(|| self.read_number(card, "mem_info_vram_total"))
                        .flatten()
                        .map(bytes_to_mb),
                    shared_used_mb: 0,
                    unified_memory: false,
                    source: GpuSource::DrmFdinfo,
                    process_load: Accuracy::Exact,
                    process_memory: Accuracy::Exact,
                }
            })
            .collect();
        self.previous = Some((now, clients));
        GpuReading {
            adapters,
            processes,
        }
    }
}

/// Every GPU in `/sys/class/drm` whose driver publishes fdinfo statistics, named from
/// `pci.ids`. Connectors (`card0-DP-1`) and render nodes are not cards.
fn cards(root: &Path) -> Vec<Card> {
    let Ok(entries) = fs::read_dir(root.join("sys/class/drm")) else {
        return Vec::new();
    };
    let mut cards: Vec<Card> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let number = name.strip_prefix("card")?;
            if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            let device = Path::new("sys/class/drm").join(&name).join("device");
            card(root, device)
        })
        .collect();
    cards.sort_by(|a, b| a.address.cmp(&b.address));
    cards.dedup_by(|a, b| a.address == b.address);
    cards
}

fn card(root: &Path, device: PathBuf) -> Option<Card> {
    let uevent = fs::read_to_string(root.join(&device).join("uevent")).ok()?;
    let field = |key: &str| {
        uevent.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name == key).then(|| value.trim().to_owned())
        })
    };
    let driver = field("DRIVER")?;
    if !DRIVERS.contains(&driver.as_str()) {
        return None;
    }
    // A platform device (most Arm GPUs) has no PCI slot; fdinfo names it like its directory.
    let address = match field("PCI_SLOT_NAME") {
        Some(slot) => pci::normalize_address(&slot).unwrap_or(slot),
        None => fs::canonicalize(root.join(&device))
            .ok()?
            .file_name()?
            .to_string_lossy()
            .into_owned(),
    };
    let ids = field("PCI_ID").and_then(|ids| {
        let (vendor, device) = ids.split_once(':')?;
        Some((
            u16::from_str_radix(vendor, 16).ok()?,
            u16::from_str_radix(device, 16).ok()?,
        ))
    });
    let name = match ids {
        Some((vendor, device)) => pci::adapter_name(root, vendor, device, &address),
        None => format!("{driver} GPU {address}"),
    };
    Some(Card {
        device,
        driver,
        address,
        name,
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::linux::{gpu::fdinfo::tests::fixture_files, tests::fixture};

    const AMD: &str = "0000:03:00.0";
    const INTEL: &str = "0000:00:02.0";
    const XE: &str = "0000:4d:00.0";

    /// Reads the `gpu/t0` machine, then `gpu/t1` one second later.
    fn read_twice() -> (GpuReading, GpuReading) {
        let mut drm = Drm::with_finder(&fixture("gpu/t0"), fixture_files).unwrap();
        let start = Instant::now();
        let first = drm.read(start);
        drm.root = fixture("gpu/t1");
        let second = drm.read(start + Duration::from_secs(1));
        (first, second)
    }

    fn adapter<'a>(reading: &'a GpuReading, luid: &str) -> &'a RawGpu {
        let adapter = reading.adapters.iter().find(|gpu| gpu.luid == luid);
        adapter.unwrap_or_else(|| panic!("no adapter {luid} in {:?}", reading.adapters))
    }

    #[test]
    fn test_cards_lists_fdinfo_drivers_only_with_names() {
        let cards = cards(&fixture("gpu/t0"));
        let listed: Vec<(&str, &str, &str)> = cards
            .iter()
            .map(|card| {
                (
                    card.address.as_str(),
                    card.driver.as_str(),
                    card.name.as_str(),
                )
            })
            .collect();
        assert_eq!(
            listed,
            [
                (INTEL, "i915", "Intel Iris Xe Graphics"),
                (AMD, "amdgpu", "AMD Radeon RX 6600/6600 XT/6600M"),
                (XE, "xe", "Intel GPU 0000:4d:00.0"),
            ]
        );
    }

    #[test]
    fn test_drm_new_is_none_without_a_supported_card() {
        assert!(Drm::new(&fixture("desktop")).is_none());
        assert!(Drm::new(&fixture("wsl")).is_none());
    }

    #[test]
    fn test_drm_first_read_has_memory_but_no_rates() {
        let (first, _) = read_twice();
        assert_eq!(first.adapters.len(), 3);
        let amd = adapter(&first, AMD);
        assert_eq!(amd.engines, EngineLoad::default());
        assert_eq!(amd.dedicated_used_mb, 1024);
        assert_eq!(amd.dedicated_total_mb, Some(8176));
        assert_eq!(amd.source, GpuSource::DrmFdinfo);
        assert_eq!(amd.process_load, Accuracy::Exact);
        assert_eq!(first.processes[&100].vram_mb, 100);
        assert_eq!(first.processes[&100].gpu_pct, 0.0);
    }

    #[test]
    fn test_drm_second_read_groups_engines_per_adapter() {
        let (_, second) = read_twice();
        let amd = adapter(&second, AMD);
        // blender 50% plus kwin 10% on gfx; sysfs reads 63% for every user's work.
        assert_eq!(amd.engines.three_d_pct, 63.0);
        assert_eq!(amd.engines.copy_pct, 1.0);
        assert_eq!(amd.engines.compute_pct, 0.0);

        let intel = adapter(&second, INTEL);
        assert_eq!(intel.engines.three_d_pct, 15.0);
        assert_eq!(intel.engines.video_pct, 30.0);
        assert_eq!(intel.dedicated_used_mb, 0);
        assert_eq!(intel.dedicated_total_mb, None);

        let xe = adapter(&second, XE);
        assert_eq!(xe.engines.compute_pct, 40.0);
        assert_eq!(xe.engines.three_d_pct, 0.0);
        assert_eq!(xe.dedicated_used_mb, 512);
    }

    #[test]
    fn test_drm_second_read_splits_shared_clients_between_processes() {
        let (_, second) = read_twice();
        // blender and its helper share client 23: 50% gfx and 200 MiB, half each.
        assert_eq!(second.processes[&100].gpu_pct, 25.0);
        assert_eq!(second.processes[&101].gpu_pct, 25.0);
        assert_eq!(second.processes[&100].vram_mb, 100);
        assert_eq!(second.processes[&200].gpu_pct, 30.0);
        assert_eq!(second.processes[&300].gpu_pct, 40.0);
        assert_eq!(second.processes[&300].vram_mb, 512);
        assert_eq!(second.processes[&400].gpu_pct, 10.0);
        // The NVIDIA driver's files carry no statistics.
        assert!(!second.processes.contains_key(&500));
    }

    #[test]
    fn test_engine_group_of_driver_engine_names() {
        assert_eq!(EngineGroup::of("gfx"), Some(EngineGroup::ThreeD));
        assert_eq!(EngineGroup::of("ccs"), Some(EngineGroup::Compute));
        assert_eq!(EngineGroup::of("bcs"), Some(EngineGroup::Copy));
        assert_eq!(EngineGroup::of("video-enhance"), Some(EngineGroup::Video));
        assert_eq!(EngineGroup::of("vecs"), Some(EngineGroup::Video));
        assert_eq!(EngineGroup::of("enc_1"), Some(EngineGroup::Video));
        assert_eq!(EngineGroup::of("security"), None);
    }
}
