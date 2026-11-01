//! Processes the app starts. Each runs in its own process group, through the
//! user's login shell, with stdout and stderr captured into a ring buffer.
//!
//! Stopping signals the whole group, so `pnpm dev` takes its vite child down
//! with it. Dropping the last `Supervisor` clone stops everything it started.
//!
//! With `with_runs`, a process can instead run in a tmux session
//! (`start_tmux`, see `tmux_runs.rs`): attachable from any terminal and
//! left running when the app quits. The other calls cover both kinds.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const LOG_LINES: usize = 2000;
const STOP_GRACE: Duration = Duration::from_secs(2);

type Log = Arc<Mutex<VecDeque<String>>>;

struct Managed {
    child: Child,
    log: Log,
}

#[derive(Default)]
struct Inner {
    running: HashMap<String, Managed>,
    /// Logs outlive their process, so a crash's last lines stay readable.
    logs: HashMap<String, Log>,
    /// Exit codes of processes that ended on their own (not stopped here),
    /// until `take_exits` reads them. None: killed by a signal.
    exits: Vec<(String, Option<i32>)>,
    /// tmux runs whose end was already reported (or found ended at start).
    reported_dead: std::collections::HashSet<String>,
    /// The tmux runs' panes, refreshed at most every `RUNS_FRESH`.
    runs_cache: Option<(Instant, HashMap<String, crate::tmux_runs::PaneState>)>,
}

/// How long a read of the tmux runs is reused (renders ask per script).
const RUNS_FRESH: Duration = Duration::from_secs(1);

#[derive(Clone, Default)]
pub struct Supervisor {
    inner: Arc<Mutex<Inner>>,
    runs: Option<crate::tmux_runs::Runs>,
}

pub fn shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into())
}

fn signal_group(pgid: u32, signal: &str) {
    let _ = Command::new("kill")
        .args([&format!("-{signal}"), "--", &format!("-{pgid}")])
        .stderr(Stdio::null())
        .status();
}

fn push_line(log: &Log, line: String) {
    let mut log = log.lock().unwrap();
    if log.len() == LOG_LINES {
        log.pop_front();
    }
    log.push_back(line);
}

fn pump(reader: impl Read + Send + 'static, log: Log, prefix: &'static str) {
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            match line {
                Ok(line) => push_line(&log, format!("{prefix}{line}")),
                Err(_) => break,
            }
        }
    });
}

fn stop_child(mut managed: Managed) {
    let pgid = managed.child.id();
    signal_group(pgid, "TERM");
    let deadline = Instant::now() + STOP_GRACE;
    while Instant::now() < deadline {
        if managed.child.try_wait().ok().flatten().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // The leader may be gone while group members linger: always finish the group.
    signal_group(pgid, "KILL");
    let _ = managed.child.wait();
    push_line(&managed.log, "── stopped ──".into());
}

impl Supervisor {
    pub fn new() -> Self {
        Self::default()
    }

    /// A supervisor that can also run in tmux sessions. Runs that already
    /// ended (before this app started) are not reported again.
    pub fn with_runs(runs: crate::tmux_runs::Runs) -> Self {
        let ended: std::collections::HashSet<String> = runs
            .all_states()
            .into_iter()
            .filter(|(_, s)| s.dead)
            .map(|(k, _)| k)
            .collect();
        let supervisor = Self {
            inner: Default::default(),
            runs: Some(runs),
        };
        supervisor.inner.lock().unwrap().reported_dead = ended;
        supervisor
    }

    /// The tmux runs' panes (cached for `RUNS_FRESH`).
    fn run_states(&self) -> HashMap<String, crate::tmux_runs::PaneState> {
        let Some(runs) = &self.runs else {
            return HashMap::new();
        };
        {
            let inner = self.inner.lock().unwrap();
            if let Some((at, states)) = &inner.runs_cache
                && at.elapsed() < RUNS_FRESH
            {
                return states.clone();
            }
        }
        let states = runs.all_states();
        self.inner.lock().unwrap().runs_cache = Some((Instant::now(), states.clone()));
        states
    }

    fn forget_run_states(&self) {
        self.inner.lock().unwrap().runs_cache = None;
    }

    /// Starts `command` in a tmux session named for `key`; the session name.
    pub fn start_tmux(
        &self,
        key: &str,
        command: &str,
        cwd: &Path,
        label: &str,
    ) -> Result<String, String> {
        let runs = self
            .runs
            .as_ref()
            .ok_or("tmux runs are not available (is tmux installed?)")?;
        if self.is_running(key) {
            return Err(format!("{key} is already running"));
        }
        let name = runs.start(key, command, cwd, label)?;
        let mut inner = self.inner.lock().unwrap();
        inner.reported_dead.remove(key);
        inner.runs_cache = None;
        Ok(name)
    }

    /// The tmux session `key` runs (or ran) in, if it has one.
    pub fn tmux_session(&self, key: &str) -> Option<String> {
        self.run_states()
            .contains_key(key)
            .then(|| crate::tmux_runs::session_name(key))
    }

    /// Starts `command` under the login shell in `cwd`. Returns the PID
    /// (also the process group id).
    pub fn start(&self, key: &str, command: &str, cwd: &Path) -> Result<u32, String> {
        if self.is_running(key) {
            return Err(format!("{key} is already running"));
        }
        let mut child = Command::new(shell())
            .args(["-lc", command])
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .spawn()
            .map_err(|e| format!("{command}: {e}"))?;
        let pid = child.id();

        let mut inner = self.inner.lock().unwrap();
        let log = inner.logs.entry(key.to_string()).or_default().clone();
        push_line(&log, format!("── $ {command}  (pid {pid}) ──"));
        if let Some(stdout) = child.stdout.take() {
            pump(stdout, log.clone(), "");
        }
        if let Some(stderr) = child.stderr.take() {
            pump(stderr, log.clone(), "");
        }
        inner
            .running
            .insert(key.to_string(), Managed { child, log });
        Ok(pid)
    }

    pub fn stop(&self, key: &str) {
        let managed = self.inner.lock().unwrap().running.remove(key);
        if let Some(managed) = managed {
            stop_child(managed);
        }
        if let Some(runs) = &self.runs
            && self.run_states().contains_key(key)
        {
            runs.stop(key);
            let mut inner = self.inner.lock().unwrap();
            inner.reported_dead.insert(key.to_string());
            inner.runs_cache = None;
        }
    }

    /// Restarts where it ran: in its tmux session, or in the background.
    pub fn restart(&self, key: &str, command: &str, cwd: &Path) -> Result<u32, String> {
        if self.runs.is_some() && self.run_states().contains_key(key) {
            self.stop(key);
            let label = key
                .rsplit_once("::")
                .map_or(key, |(_, name)| name)
                .to_string();
            self.start_tmux(key, command, cwd, &label)?;
            self.forget_run_states();
            return Ok(self.pid(key).unwrap_or(0));
        }
        self.stop(key);
        self.start(key, command, cwd)
    }

    /// Reaps exited children; true while the process is alive.
    pub fn is_running(&self, key: &str) -> bool {
        if let Some(state) = self.run_states().get(key).copied() {
            if !state.dead {
                return true;
            }
            let mut inner = self.inner.lock().unwrap();
            if inner.reported_dead.insert(key.to_string()) {
                inner.exits.push((key.to_string(), state.exit));
            }
            return false;
        }
        let mut inner = self.inner.lock().unwrap();
        let Some(managed) = inner.running.get_mut(key) else {
            return false;
        };
        match managed.child.try_wait() {
            Ok(None) => true,
            Ok(Some(status)) => {
                let managed = inner.running.remove(key).unwrap();
                // Take down anything the leader left behind in its group.
                signal_group(managed.child.id(), "KILL");
                push_line(&managed.log, format!("── exited: {status} ──"));
                inner.exits.push((key.to_string(), status.code()));
                false
            }
            Err(_) => false,
        }
    }

    pub fn pid(&self, key: &str) -> Option<u32> {
        if !self.is_running(key) {
            return None;
        }
        if let Some(state) = self.run_states().get(key) {
            return Some(state.pid);
        }
        self.inner
            .lock()
            .unwrap()
            .running
            .get(key)
            .map(|m| m.child.id())
    }

    /// The file a tmux run logs to, when it has one.
    pub fn log_file(&self, key: &str) -> Option<std::path::PathBuf> {
        let path = self.runs.as_ref()?.log_path(key);
        path.exists().then_some(path)
    }

    pub fn logs(&self, key: &str) -> Vec<String> {
        if let Some(runs) = &self.runs
            && (self.run_states().contains_key(key)
                || (runs.log_path(key).exists()
                    && !self.inner.lock().unwrap().logs.contains_key(key)))
        {
            return runs.read_log(key, LOG_LINES);
        }
        let inner = self.inner.lock().unwrap();
        inner
            .logs
            .get(key)
            .map(|log| log.lock().unwrap().iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Every key with a log, running or not, that starts with `prefix`.
    pub fn log_keys(&self, prefix: &str) -> Vec<String> {
        let mut keys: Vec<String> = self
            .inner
            .lock()
            .unwrap()
            .logs
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        for key in self.run_states().into_keys() {
            if key.starts_with(prefix) && !keys.contains(&key) {
                keys.push(key);
            }
        }
        keys.sort();
        keys
    }

    pub fn clear_logs(&self, key: &str) {
        if let Some(log) = self.inner.lock().unwrap().logs.get(key) {
            log.lock().unwrap().clear();
        }
        if let Some(runs) = &self.runs {
            runs.clear_log(key);
        }
    }

    /// Processes that exited on their own since the last call.
    pub fn take_exits(&self) -> Vec<(String, Option<i32>)> {
        std::mem::take(&mut self.inner.lock().unwrap().exits)
    }

    pub fn stop_all(&self) {
        let running: Vec<Managed> = self
            .inner
            .lock()
            .unwrap()
            .running
            .drain()
            .map(|(_, m)| m)
            .collect();
        for managed in running {
            stop_child(managed);
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        for (_, managed) in self.running.drain() {
            signal_group(managed.child.id(), "TERM");
        }
    }
}

/// Sends SIGTERM to a process the app did not start (a port conflict).
pub fn kill_external(pid: u32) -> Result<(), String> {
    let status = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("kill {pid} failed"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::process_alive;

    fn wait_until(mut f: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        false
    }

    #[test]
    fn captures_stdout_and_stderr() {
        // Given
        let sup = Supervisor::new();

        // When
        sup.start("t", "echo out; echo err 1>&2", Path::new("/"))
            .unwrap();

        // Then
        assert!(wait_until(|| {
            let logs = sup.logs("t");
            logs.iter().any(|l| l == "out") && logs.iter().any(|l| l == "err")
        }));
    }

    #[test]
    fn stop_kills_process_group() {
        // Given
        let sup = Supervisor::new();
        let dir = tempfile::tempdir().unwrap();
        let pids = dir.path().join("pids");
        let command = format!("sleep 30 & echo $! > {}; sleep 30", pids.display());
        sup.start("t", &command, Path::new("/")).unwrap();
        assert!(wait_until(
            || std::fs::read_to_string(&pids).is_ok_and(|s| !s.trim().is_empty())
        ));
        let background: u32 = std::fs::read_to_string(&pids)
            .unwrap()
            .trim()
            .parse()
            .unwrap();

        // When
        sup.stop("t");

        // Then
        assert!(!sup.is_running("t"));
        assert!(wait_until(|| !process_alive(background)));
    }

    #[test]
    fn ring_buffer_caps_lines() {
        // Given
        let sup = Supervisor::new();
        let command = format!(
            "i=0; while [ $i -lt {} ]; do echo $i; i=$((i+1)); done",
            LOG_LINES + 50
        );

        // When
        sup.start("t", &command, Path::new("/")).unwrap();

        // Then
        let last = (LOG_LINES + 49).to_string();
        assert!(wait_until(|| sup.logs("t").last() == Some(&last)));
        assert_eq!(sup.logs("t").len(), LOG_LINES);
    }

    #[test]
    fn natural_exit_codes_are_reported_once() {
        // Given
        let sup = Supervisor::new();
        sup.start("ok", "exit 0", Path::new("/")).unwrap();
        sup.start("bad", "exit 3", Path::new("/")).unwrap();

        // When
        assert!(wait_until(
            || !sup.is_running("ok") && !sup.is_running("bad")
        ));
        let mut exits = sup.take_exits();
        exits.sort();
        let again = sup.take_exits();

        // Then
        assert_eq!(
            exits,
            [("bad".to_string(), Some(3)), ("ok".to_string(), Some(0))]
        );
        assert!(again.is_empty());
    }

    #[test]
    fn exited_process_not_running() {
        // Given
        let sup = Supervisor::new();

        // When
        sup.start("t", "true", Path::new("/")).unwrap();

        // Then
        assert!(wait_until(|| !sup.is_running("t")));
        assert!(wait_until(|| sup
            .logs("t")
            .iter()
            .any(|l| l.contains("exited"))));
    }
}
