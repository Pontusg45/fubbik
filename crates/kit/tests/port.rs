use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use kit::port::{self, KillOutcome};

fn have_lsof() -> bool {
    Command::new("lsof")
        .arg("-v")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// Reaps the child on every path, including assertion failure.
struct Listener(Child);

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_listener(port: u16) -> Listener {
    let code = format!(
        "import socket,time; s=socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(('127.0.0.1', {port})); s.listen(); time.sleep(30)"
    );
    let child = Command::new("python3")
        .args(["-c", &code])
        .spawn()
        .expect("spawn python3");
    let guard = Listener(child);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if !port::pids(port).unwrap().is_empty() {
            return guard;
        }
        assert!(
            Instant::now() < deadline,
            "listener never bound port {port}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn free_port_is_not_an_error() {
    if !have_lsof() {
        return;
    }
    // Given a port nothing listens on
    let port = free_port();

    // When killing it
    let outcome = port::kill(port, false).unwrap();

    // Then it is reported free, not an error
    assert_eq!(outcome, KillOutcome::Free);
}

#[test]
fn cli_free_port_works_outside_a_git_repo() {
    if !have_lsof() {
        return;
    }
    // Given an empty directory that is not a git repo and a free port
    let dir = std::env::temp_dir().join(format!("fk-port-nogit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let port = free_port();

    // When running `fk port kill` from that directory
    let out = Command::new(env!("CARGO_BIN_EXE_fk"))
        .args(["port", "kill", &port.to_string()])
        .current_dir(&dir)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    // Then it exits 0 and says the port is free
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        format!("port {port} is free")
    );
}

#[test]
fn kills_listener_on_port() {
    if !have_lsof() {
        return;
    }
    // Given a process listening on a port
    let port = free_port();
    let mut l = spawn_listener(port);
    let pid = l.0.id();

    // When killing the port
    let outcome = port::kill(port, false).unwrap();

    // Then the listener's pid is reported killed and the child died
    match outcome {
        KillOutcome::Killed { pids } => assert!(pids.contains(&pid)),
        other => panic!("expected Killed, got {other:?}"),
    }
    let status = l.0.wait().unwrap();
    assert!(!status.success());
}

#[test]
fn dry_run_keeps_listener_alive() {
    if !have_lsof() {
        return;
    }
    // Given a process listening on a port
    let port = free_port();
    let mut l = spawn_listener(port);
    let pid = l.0.id();

    // When doing a dry run
    let outcome = port::kill(port, true).unwrap();

    // Then it reports WouldKill and the child keeps running
    match outcome {
        KillOutcome::WouldKill { pids } => assert!(pids.contains(&pid)),
        other => panic!("expected WouldKill, got {other:?}"),
    }
    assert!(l.0.try_wait().unwrap().is_none(), "child should still run");
}

#[test]
fn does_not_kill_clients_of_a_listener() {
    if !have_lsof() {
        return;
    }
    // Given a listener on a port and this test process connected to it as a client
    let port = free_port();
    let l = spawn_listener(port);
    let _client = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();

    // When looking up the pids on that port
    let pids = port::pids(port).unwrap();

    // Then only the listener is included, not this client process
    assert!(pids.contains(&l.0.id()));
    assert!(!pids.contains(&std::process::id()));
}
