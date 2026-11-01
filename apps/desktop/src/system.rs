//! The computer's own state for the Projects page (and MCP): CPU, memory,
//! disk, battery, network, uptime, the busiest processes and which ports
//! are held by whom. A background thread samples it; readers take the
//! latest `Stats` without waiting.

use std::collections::VecDeque;
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use sysinfo::{Disks, Networks, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};

/// Samples kept for the CPU and memory graphs (two minutes at `EVERY`).
const HISTORY: usize = 60;
const EVERY: Duration = Duration::from_secs(2);
const SLOW_EVERY: Duration = Duration::from_secs(30);
const PORTS_EVERY: Duration = Duration::from_secs(6);

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Proc {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiskSpace {
    pub mount: String,
    pub free: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Battery {
    pub percent: u8,
    /// "charging", "discharging", "charged", "AC attached"…
    pub state: String,
    pub remaining: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Port {
    pub port: u16,
    pub pid: u32,
    pub command: String,
}

#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub host: String,
    pub os: String,
    pub cpu_brand: String,
    /// Percent, all cores together.
    pub cpu: f32,
    pub cores: Vec<f32>,
    pub cpu_history: Vec<f32>,
    pub mem_used: u64,
    pub mem_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    pub mem_history: Vec<f32>,
    pub load: (f64, f64, f64),
    pub uptime: u64,
    pub disk: Option<DiskSpace>,
    pub battery: Option<Battery>,
    /// Bytes per second over the last sample.
    pub net_rx: u64,
    pub net_tx: u64,
    pub top_cpu: Vec<Proc>,
    pub top_memory: Vec<Proc>,
    pub ports: Vec<Port>,
    /// None until the first sample.
    pub at: Option<Instant>,
}

impl Stats {
    pub fn mem_percent(&self) -> f32 {
        percent(self.mem_used, self.mem_total)
    }
}

pub fn percent(part: u64, whole: u64) -> f32 {
    if whole == 0 {
        0.0
    } else {
        part as f32 * 100.0 / whole as f32
    }
}

fn cell() -> &'static Mutex<Stats> {
    static STATS: OnceLock<Mutex<Stats>> = OnceLock::new();
    STATS.get_or_init(Default::default)
}

/// The latest sample.
pub fn stats() -> Stats {
    cell().lock().unwrap().clone()
}

/// Starts sampling on a background thread (once).
pub fn start() {
    static STARTED: OnceLock<()> = OnceLock::new();
    if STARTED.set(()).is_err() {
        return;
    }
    let _ = std::thread::Builder::new()
        .name("fubbik-system".into())
        .spawn(sample_forever);
}

fn sample_forever() {
    let mut system = System::new_with_specifics(RefreshKind::everything());
    let mut networks = Networks::new_with_refreshed_list();
    let mut disks = Disks::new_with_refreshed_list();
    let (mut cpu_history, mut mem_history) = (VecDeque::new(), VecDeque::new());
    let (mut slow_at, mut ports_at): (Option<Instant>, Option<Instant>) = (None, None);
    let (mut battery, mut disk, mut ports) = (None, None, Vec::new());
    let started = Instant::now();
    loop {
        std::thread::sleep(EVERY);
        system.refresh_cpu_usage();
        system.refresh_memory();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        networks.refresh(true);
        if slow_at.is_none_or(|at| at.elapsed() >= SLOW_EVERY) {
            disks.refresh(true);
            disk = data_disk(&disks);
            battery = read_battery();
            slow_at = Some(Instant::now());
        }
        if ports_at.is_none_or(|at| at.elapsed() >= PORTS_EVERY) {
            ports = read_ports();
            ports_at = Some(Instant::now());
        }

        let cpu = system.global_cpu_usage();
        push(&mut cpu_history, cpu);
        let mem = percent(system.used_memory(), system.total_memory());
        push(&mut mem_history, mem);
        let seconds = EVERY.as_secs_f64().max(0.001);
        let (rx, tx) = networks
            .iter()
            .filter(|(name, _)| !name.starts_with("lo"))
            .fold((0, 0), |(rx, tx), (_, n)| {
                (rx + n.received(), tx + n.transmitted())
            });
        // The first sample has no previous one to compare against.
        let warm = started.elapsed() > EVERY * 2;
        let mut procs: Vec<Proc> = system
            .processes()
            .iter()
            .map(|(pid, p)| Proc {
                pid: pid.as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                cpu: p.cpu_usage(),
                memory: p.memory(),
            })
            .collect();
        procs.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));
        let top_cpu = procs.iter().take(6).cloned().collect();
        procs.sort_by_key(|p| std::cmp::Reverse(p.memory));
        let top_memory = procs.iter().take(6).cloned().collect();
        let load = System::load_average();

        *cell().lock().unwrap() = Stats {
            host: System::host_name().unwrap_or_default(),
            os: System::long_os_version().unwrap_or_default(),
            cpu_brand: system
                .cpus()
                .first()
                .map(|c| c.brand().to_string())
                .unwrap_or_default(),
            cpu,
            cores: system.cpus().iter().map(|c| c.cpu_usage()).collect(),
            cpu_history: cpu_history.iter().copied().collect(),
            mem_used: system.used_memory(),
            mem_total: system.total_memory(),
            swap_used: system.used_swap(),
            swap_total: system.total_swap(),
            mem_history: mem_history.iter().copied().collect(),
            load: (load.one, load.five, load.fifteen),
            uptime: System::uptime(),
            disk: disk.clone(),
            battery: battery.clone(),
            net_rx: if warm {
                (rx as f64 / seconds) as u64
            } else {
                0
            },
            net_tx: if warm {
                (tx as f64 / seconds) as u64
            } else {
                0
            },
            top_cpu,
            top_memory,
            ports: ports.clone(),
            at: Some(Instant::now()),
        };
    }
}

fn push(history: &mut VecDeque<f32>, value: f32) {
    if history.len() == HISTORY {
        history.pop_front();
    }
    history.push_back(value);
}

/// The volume user files live on: macOS's data volume, else `/`.
fn data_disk(disks: &Disks) -> Option<DiskSpace> {
    let pick = |mount: &str| {
        disks
            .iter()
            .find(|d| d.mount_point().to_string_lossy() == mount)
    };
    let disk = pick("/System/Volumes/Data").or_else(|| pick("/"))?;
    Some(DiskSpace {
        mount: disk.mount_point().to_string_lossy().into_owned(),
        free: disk.available_space(),
        total: disk.total_space(),
    })
}

fn read_battery() -> Option<Battery> {
    let out = Command::new("pmset").args(["-g", "batt"]).output().ok()?;
    parse_battery(&String::from_utf8_lossy(&out.stdout))
}

/// `pmset -g batt`: " -InternalBattery-0 (id=…)\t87%; charging; 1:02 remaining present: true"
pub fn parse_battery(text: &str) -> Option<Battery> {
    let line = text.lines().find(|l| l.contains("InternalBattery"))?;
    let fields = line.split('\t').nth(1)?;
    let mut parts = fields.split(';').map(str::trim);
    let percent = parts.next()?.trim_end_matches('%').parse().ok()?;
    let state = parts.next().unwrap_or("").to_string();
    let remaining = parts
        .next()
        .and_then(|r| r.split_whitespace().next())
        .filter(|r| r.contains(':') && *r != "0:00")
        .map(str::to_string);
    Some(Battery {
        percent,
        state,
        remaining,
    })
}

fn read_ports() -> Vec<Port> {
    let Ok(out) = Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-Fpcn"])
        .output()
    else {
        return Vec::new();
    };
    parse_ports(&String::from_utf8_lossy(&out.stdout))
}

/// `lsof -F pcn`: `p<pid>`, `c<command>`, then `n<addr>:<port>` lines.
pub fn parse_ports(text: &str) -> Vec<Port> {
    let (mut pid, mut command) = (0, String::new());
    let mut ports: Vec<Port> = Vec::new();
    for line in text.lines() {
        match line.split_at_checked(1) {
            Some(("p", rest)) => pid = rest.parse().unwrap_or(0),
            Some(("c", rest)) => command = rest.to_string(),
            Some(("n", rest)) => {
                let Some(port) = rest.rsplit(':').next().and_then(|p| p.parse().ok()) else {
                    continue;
                };
                if !ports.iter().any(|p| p.port == port && p.pid == pid) {
                    ports.push(Port {
                        port,
                        pid,
                        command: command.clone(),
                    });
                }
            }
            _ => {}
        }
    }
    ports.sort_by_key(|p| p.port);
    ports
}

/// `1.2 GB`, `340 MB`, `12 KB`.
pub fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit >= 3 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

/// `3d 4h`, `5h 12m`, `9m`.
pub fn duration(seconds: u64) -> String {
    let (days, hours, minutes) = (
        seconds / 86_400,
        seconds % 86_400 / 3600,
        seconds % 3600 / 60,
    );
    match (days, hours) {
        (0, 0) => format!("{minutes}m"),
        (0, _) => format!("{hours}h {minutes}m"),
        _ => format!("{days}d {hours}h"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_battery_line() {
        // Given
        let text = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=123)\t87%; discharging; 4:12 remaining present: true\n";

        // When
        let battery = parse_battery(text);

        // Then
        assert_eq!(
            battery,
            Some(Battery {
                percent: 87,
                state: "discharging".into(),
                remaining: Some("4:12".into())
            })
        );
        assert_eq!(parse_battery("Now drawing from 'AC Power'\n"), None);
    }

    #[test]
    fn reads_listening_ports_once_each() {
        // Given: postgres listens on IPv4 and IPv6
        let text = "p101\ncpostgres\nn127.0.0.1:5432\nn[::1]:5432\np202\ncnode\nn*:3001\n";

        // When
        let ports = parse_ports(text);

        // Then
        assert_eq!(
            ports,
            [
                Port {
                    port: 3001,
                    pid: 202,
                    command: "node".into()
                },
                Port {
                    port: 5432,
                    pid: 101,
                    command: "postgres".into()
                }
            ]
        );
    }

    #[test]
    fn sizes_and_durations_read_naturally() {
        // Given
        let sizes = [512, 2048, 5 * 1024 * 1024, 3 * 1024 * 1024 * 1024 / 2];

        // When
        let shown: Vec<String> = sizes.iter().map(|n| bytes(*n)).collect();

        // Then
        assert_eq!(shown, ["512 B", "2 KB", "5 MB", "1.5 GB"]);
        assert_eq!(
            [
                duration(540),
                duration(5 * 3600 + 720),
                duration(3 * 86_400 + 4 * 3600)
            ],
            ["9m", "5h 12m", "3d 4h"]
        );
    }
}
