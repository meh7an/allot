//! DRM client statistics from `/proc/<pid>/fdinfo/<fd>`, in the format of the kernel's
//! drm-usage-stats document: `drm-engine-<engine>` busy time in ns (with an optional
//! `drm-engine-capacity-<engine>`), or `drm-cycles-<engine>` with `drm-total-cycles-<engine>`,
//! and memory per region.

use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::Path,
};

use crate::linux::gpu::pci;

/// One open file of a process: `/proc/<pid>/fdinfo/<fd>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenFile {
    pub(crate) pid: u32,
    pub(crate) fd: String,
}

/// Which DRM client a file belongs to. Duplicated fds and fds passed to another process
/// share one client, so its usage counts once.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ClientKey {
    pub(crate) driver: String,
    /// The device, as [`pci::normalize_address`] spells a PCI address.
    pub(crate) pdev: String,
    /// `drm-client-id`, or the pid and fd for a driver that leaves it out, so such a client
    /// is never merged with another by mistake.
    id: String,
}

/// Usage counters of one DRM client.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Client {
    /// Every process holding the client open.
    pub(crate) pids: BTreeSet<u32>,
    /// Busy time per engine, in ns.
    engine_ns: HashMap<String, u64>,
    /// Engines of each kind that `engine_ns` adds up; 1 when not listed.
    capacity: HashMap<String, u64>,
    /// Busy cycles per engine, for drivers that count cycles instead of time.
    cycles: HashMap<String, u64>,
    /// Cycles the engine ran in total, busy or not.
    total_cycles: HashMap<String, u64>,
    /// Bytes resident in the device's own memory.
    pub(crate) vram_bytes: u64,
}

pub(crate) type Clients = HashMap<ClientKey, Client>;

impl Client {
    /// Busy percentage per engine since `previous`, `wall_ns` earlier. A counter that went
    /// back counts as idle: the format lets drivers report a lagging value.
    pub(crate) fn engine_busy(&self, previous: &Self, wall_ns: u64) -> HashMap<String, f32> {
        let mut busy = HashMap::new();
        if wall_ns > 0 {
            for (engine, &ns) in &self.engine_ns {
                let Some(&before) = previous.engine_ns.get(engine) else {
                    continue;
                };
                let capacity = self.capacity.get(engine).copied().unwrap_or(1);
                let ratio = ns.saturating_sub(before) as f64 / (wall_ns * capacity) as f64;
                busy.insert(engine.clone(), to_pct(ratio));
            }
        }
        for (engine, &cycles) in &self.cycles {
            let (Some(&before), Some(&total), Some(&total_before)) = (
                previous.cycles.get(engine),
                self.total_cycles.get(engine),
                previous.total_cycles.get(engine),
            ) else {
                continue;
            };
            let elapsed = total.saturating_sub(total_before);
            if elapsed > 0 {
                let ratio = cycles.saturating_sub(before) as f64 / elapsed as f64;
                busy.insert(engine.clone(), to_pct(ratio));
            }
        }
        busy
    }
}

fn to_pct(ratio: f64) -> f32 {
    (ratio * 100.0).clamp(0.0, 100.0) as f32
}

/// Every open file of every process under `root` that points into `/dev/dri/`. Processes
/// of other users cannot be read and are left out.
pub(crate) fn drm_files(root: &Path) -> Vec<OpenFile> {
    let mut files = Vec::new();
    let Ok(processes) = fs::read_dir(root.join("proc")) else {
        return files;
    };
    for process in processes.flatten() {
        let Some(pid) = process
            .file_name()
            .to_str()
            .and_then(|pid| pid.parse().ok())
        else {
            continue;
        };
        let Ok(fds) = fs::read_dir(process.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            let points_to_dri =
                fs::read_link(fd.path()).is_ok_and(|target| target.starts_with("/dev/dri/"));
            if points_to_dri {
                files.push(OpenFile {
                    pid,
                    fd: fd.file_name().to_string_lossy().into_owned(),
                });
            }
        }
    }
    files
}

/// The clients behind `files`, read from their fdinfo under `root`. Files that are gone
/// or carry no DRM statistics are skipped.
pub(crate) fn read_clients(root: &Path, files: &[OpenFile]) -> Clients {
    let mut clients = Clients::new();
    for file in files {
        let path = root
            .join("proc")
            .join(file.pid.to_string())
            .join("fdinfo")
            .join(&file.fd);
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let Some((key, client)) = parse(&text, file) else {
            continue;
        };
        clients.entry(key).or_insert(client).pids.insert(file.pid);
    }
    clients
}

/// The client in one fdinfo file, or `None` when it has no `drm-driver`.
fn parse(text: &str, file: &OpenFile) -> Option<(ClientKey, Client)> {
    let mut client = Client::default();
    let (mut driver, mut pdev, mut id) = (None, None, None);
    let mut resident_vram = None;
    let mut legacy_vram = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let Some(key) = key.strip_prefix("drm-") else {
            continue;
        };
        let value = value.trim();
        if let Some(engine) = key.strip_prefix("engine-capacity-") {
            if let Some(capacity) = number(value) {
                client.capacity.insert(engine.to_owned(), capacity.max(1));
            }
        } else if let Some(engine) = key.strip_prefix("engine-") {
            insert_number(&mut client.engine_ns, engine, value);
        } else if let Some(engine) = key.strip_prefix("total-cycles-") {
            insert_number(&mut client.total_cycles, engine, value);
        } else if let Some(engine) = key.strip_prefix("cycles-") {
            insert_number(&mut client.cycles, engine, value);
        } else if let Some(region) = key.strip_prefix("resident-") {
            add_vram(&mut resident_vram, region, value);
        } else if let Some(region) = key.strip_prefix("memory-") {
            // amdgpu's older name for resident memory.
            add_vram(&mut legacy_vram, region, value);
        } else {
            match key {
                "driver" => driver = Some(value.to_owned()),
                "pdev" => {
                    pdev = Some(pci::normalize_address(value).unwrap_or_else(|| value.to_owned()));
                }
                "client-id" => id = Some(value.to_owned()),
                _ => {}
            }
        }
    }
    client.vram_bytes = resident_vram.or(legacy_vram).unwrap_or_default();
    let key = ClientKey {
        driver: driver?,
        pdev: pdev.unwrap_or_default(),
        id: id.unwrap_or_else(|| format!("pid {} fd {}", file.pid, file.fd)),
    };
    Some((key, client))
}

fn insert_number(map: &mut HashMap<String, u64>, engine: &str, value: &str) {
    if let Some(number) = number(value) {
        map.insert(engine.to_owned(), number);
    }
}

/// Adds a region's bytes when it is the device's own memory: `vram` on amdgpu and xe,
/// `local` on i915. System memory regions are RAM, which process memory already counts.
fn add_vram(total: &mut Option<u64>, region: &str, value: &str) {
    if (region.starts_with("vram") || region.starts_with("local"))
        && let Some(bytes) = number(value)
    {
        *total = Some(total.unwrap_or_default() + bytes);
    }
}

/// The number in a value such as `1500 ns`, `4096 KiB` or `2`, with sizes in bytes.
fn number(value: &str) -> Option<u64> {
    let mut parts = value.split_whitespace();
    let amount: u64 = parts.next()?.parse().ok()?;
    let scale = match parts.next() {
        Some("KiB") => 1 << 10,
        Some("MiB") => 1 << 20,
        Some("GiB") => 1 << 30,
        _ => 1,
    };
    amount.checked_mul(scale)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::linux::tests::fixture;

    /// Every fdinfo file in a fixture machine. Fixtures cannot hold the fd links that
    /// [`drm_files`] follows, so they list the fdinfo files themselves.
    pub(crate) fn fixture_files(root: &Path) -> Vec<OpenFile> {
        let mut files = Vec::new();
        for process in fs::read_dir(root.join("proc")).unwrap().flatten() {
            let Ok(pid) = process.file_name().to_string_lossy().parse() else {
                continue;
            };
            let Ok(fds) = fs::read_dir(process.path().join("fdinfo")) else {
                continue;
            };
            for fd in fds.flatten() {
                let fd = fd.file_name().to_string_lossy().into_owned();
                files.push(OpenFile { pid, fd });
            }
        }
        files
    }

    fn clients(case: &str) -> Clients {
        let root = fixture(case);
        read_clients(&root, &fixture_files(&root))
    }

    fn client<'a>(clients: &'a Clients, driver: &str, id: &str) -> &'a Client {
        clients
            .iter()
            .find(|(key, _)| key.driver == driver && key.id == id)
            .map(|(_, client)| client)
            .unwrap_or_else(|| panic!("no {driver} client {id} in {clients:?}"))
    }

    #[test]
    fn test_read_clients_merges_shared_files_and_keeps_anonymous_ones_apart() {
        let clients = clients("gpu/t0");
        // blender's two fds and the fd it passed to its helper are one client.
        let blender = client(&clients, "amdgpu", "23");
        assert_eq!(blender.pids, BTreeSet::from([100, 101]));
        let anonymous: Vec<&ClientKey> = clients
            .keys()
            .filter(|key| key.id.starts_with("pid "))
            .collect();
        assert_eq!(anonymous.len(), 1, "{anonymous:?}");
        assert_eq!(anonymous[0].id, "pid 400 fd 12");
        // An fd without drm-driver (the NVIDIA driver writes none) is no client.
        assert!(clients.keys().all(|key| key.driver != "nvidia"));
        assert_eq!(clients.len(), 5);
    }

    #[test]
    fn test_parse_reads_device_memory_only() {
        let clients = clients("gpu/t0");
        assert_eq!(
            client(&clients, "amdgpu", "23").vram_bytes,
            200 * 1024 * 1024
        );
        assert_eq!(client(&clients, "xe", "3").vram_bytes, 512 * 1024 * 1024);
        // i915 on an integrated GPU has only system memory.
        assert_eq!(client(&clients, "i915", "7").vram_bytes, 0);
    }

    #[test]
    fn test_parse_normalizes_pdev_and_falls_back_to_legacy_memory_keys() {
        let file = OpenFile {
            pid: 1,
            fd: "3".into(),
        };
        let text = "drm-driver:\tamdgpu\ndrm-pdev:\t0000:0A:00.0\ndrm-client-id:\t5\n\
                    drm-memory-vram:\t1024 KiB\ndrm-memory-gtt:\t2048 KiB\n";
        let (key, client) = parse(text, &file).unwrap();
        assert_eq!(key.pdev, "0000:0a:00.0");
        assert_eq!(client.vram_bytes, 1024 * 1024);
        assert_eq!(parse("pos:\t0\nflags:\t02\n", &file), None);
    }

    #[test]
    fn test_engine_busy_divides_by_wall_time_and_capacity() {
        let before = clients("gpu/t0");
        let after = clients("gpu/t1");
        let wall_ns = 1_000_000_000;
        let busy = |driver, id| {
            client(&after, driver, id).engine_busy(client(&before, driver, id), wall_ns)
        };

        let blender = busy("amdgpu", "23");
        assert_eq!(blender["gfx"], 50.0);
        assert_eq!(blender["dma"], 1.0);
        assert_eq!(blender["compute"], 0.0);
        let ffmpeg = busy("i915", "7");
        assert_eq!(ffmpeg["render"], 15.0);
        // 600 ms of two video engines in one second.
        assert_eq!(ffmpeg["video"], 30.0);
        let python = busy("xe", "3");
        assert_eq!(python["ccs"], 40.0);
        assert_eq!(python["rcs"], 0.0);
    }

    #[test]
    fn test_engine_busy_treats_a_counter_that_went_back_as_idle_and_caps_at_100() {
        let file = OpenFile {
            pid: 1,
            fd: "3".into(),
        };
        let read = |text: &str| parse(text, &file).unwrap().1;
        let before = read("drm-driver:\tamdgpu\ndrm-engine-gfx:\t900 ns\ndrm-engine-dma:\t0 ns\n");
        let after = read(
            "drm-driver:\tamdgpu\ndrm-engine-gfx:\t500 ns\ndrm-engine-dma:\t5000 ns\n\
             drm-engine-compute:\t100 ns\n",
        );
        let busy = after.engine_busy(&before, 1_000);
        assert_eq!(busy["gfx"], 0.0);
        assert_eq!(busy["dma"], 100.0);
        // No earlier reading of the engine, so no rate yet.
        assert!(!busy.contains_key("compute"));
    }

    #[test]
    fn test_number_reads_units() {
        assert_eq!(number("1500 ns"), Some(1500));
        assert_eq!(number("4 KiB"), Some(4096));
        assert_eq!(number("2 MiB"), Some(2 << 20));
        assert_eq!(number("1 GiB"), Some(1 << 30));
        assert_eq!(number("2"), Some(2));
        assert_eq!(number("n/a"), None);
    }
}
