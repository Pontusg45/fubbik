//! Blocking status checks. They run on GPUI's background executor, never on
//! the main thread.

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const TIMEOUT: Duration = Duration::from_millis(800);

fn connect(host: &str, port: u16) -> bool {
    let Ok(addrs) = (host, port).to_socket_addrs() else {
        return false;
    };
    addrs
        .collect::<Vec<SocketAddr>>()
        .iter()
        .any(|addr| TcpStream::connect_timeout(addr, TIMEOUT).is_ok())
}

/// Something accepts connections on localhost:port (IPv4 or IPv6).
pub fn port_listening(port: u16) -> bool {
    connect("localhost", port)
}

/// The PID listening on a TCP port, via `lsof`.
pub fn port_owner(port: u16) -> Option<u32> {
    let output = Command::new("lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-t"])
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .and_then(|line| line.trim().parse().ok())
}

/// A 2xx response within the timeout.
pub fn http_ok(url: &str) -> bool {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build()
        .new_agent();
    agent.get(url).call().is_ok()
}

/// `scheme://user:pass@host:port/db?x` → `(host, port)`, with the scheme's
/// default port when none is given.
pub fn host_port_from_url(url: &str) -> Option<(String, u16)> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?']).next()?;
    let host_port = authority
        .rsplit_once('@')
        .map(|(_, hp)| hp)
        .unwrap_or(authority);
    let default = match scheme {
        "postgres" | "postgresql" => 5432,
        "mysql" => 3306,
        "redis" => 6379,
        "mongodb" => 27017,
        "http" => 80,
        "https" => 443,
        _ => return None,
    };
    if let Some(rest) = host_port.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        let port = match after.strip_prefix(':') {
            Some(port) => port.parse().ok()?,
            None => default,
        };
        return Some((host.to_string(), port));
    }
    match host_port.rsplit_once(':') {
        Some((host, port)) => Some((host.to_string(), port.parse().ok()?)),
        None => Some((host_port.to_string(), default)),
    }
}

pub fn tcp_reachable(url: &str) -> bool {
    match host_port_from_url(url) {
        Some((host, port)) => connect(&host, port),
        None => false,
    }
}

/// `Some(true)` when the compose service has a running container,
/// `None` when docker is unavailable.
pub fn compose_running(repo: &Path, service: &str) -> Option<bool> {
    let output = Command::new("docker")
        .current_dir(repo)
        .args(["compose", "ps", "--status", "running", "-q", service])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(!output.stdout.iter().all(u8::is_ascii_whitespace))
}

/// CPU percent and resident memory in KiB, from `ps`.
pub fn process_stats(pid: u32) -> Option<(f32, u64)> {
    let output = Command::new("ps")
        .args(["-o", "%cpu=,rss=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    parse_ps(&String::from_utf8_lossy(&output.stdout))
}

fn parse_ps(text: &str) -> Option<(f32, u64)> {
    let mut parts = text.split_whitespace();
    let cpu = parts.next()?.replace(',', ".").parse().ok()?;
    let rss = parts.next()?.parse().ok()?;
    Some((cpu, rss))
}

#[cfg(test)]
pub fn process_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn listening_port_detected() {
        // Given
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();

        // When
        let listening = port_listening(port);

        // Then
        assert!(listening);
    }

    #[test]
    fn free_port_not_listening() {
        // Given: ports just freed (another test running at the same time can
        // take one in between, so a few are tried)
        let free = || {
            TcpListener::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port()
        };

        // When
        let seen_free = (0..5).any(|_| !port_listening(free()));

        // Then
        assert!(seen_free);
    }

    #[test]
    fn parses_postgres_url_host_port() {
        // Given
        let urls = [
            "postgres://u:p@localhost:5434/db",
            "postgresql://localhost/db",
            "redis://cache:6380",
        ];

        // When
        let parsed: Vec<_> = urls.iter().map(|u| host_port_from_url(u)).collect();

        // Then
        assert_eq!(parsed[0], Some(("localhost".into(), 5434)));
        assert_eq!(parsed[1], Some(("localhost".into(), 5432)));
        assert_eq!(parsed[2], Some(("cache".into(), 6380)));
    }

    #[test]
    fn http_ok_against_local_server() {
        // Given
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok");
        });

        // When
        let ok = http_ok(&format!("http://127.0.0.1:{port}/health"));

        // Then
        assert!(ok);
    }

    #[test]
    fn parses_ps_output() {
        // Given
        let text = "  12,5  20480\n";

        // When
        let stats = parse_ps(text);

        // Then
        assert_eq!(stats, Some((12.5, 20480)));
    }
}
