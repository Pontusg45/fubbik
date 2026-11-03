//! Local `.test` domains through Caddy, and port conflicts.
//!
//! Each process with a port can get a domain, `<process>.<repo>.test`,
//! served by Caddy with its own local CA (`tls internal`) and proxied to
//! `localhost:<port>`. The app keeps its sites in one marked block of the
//! Caddyfile (`~/.config/caddy/Caddyfile`) and leaves the rest alone; with
//! `caddy run --watch` a write reloads Caddy. The names need `/etc/hosts`
//! entries, which take sudo, so the app only prepares that command for a
//! terminal tab.
//!
//! Port conflicts: two processes (in one repo or across repos) that declare
//! the same port cannot run at once.

use std::collections::BTreeMap;
use std::path::PathBuf;

const BEGIN: &str =
    "# fubbik-desktop begin (managed by the fubbik desktop app; edits here are replaced)";
const END: &str = "# fubbik-desktop end";

/// A process with a port, in a repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    pub repo: String,
    pub process: String,
    pub port: u16,
}

impl Site {
    /// `repo/process`, how the app's config names it.
    pub fn key(&self) -> String {
        format!("{}/{}", self.repo, self.process)
    }

    /// `web.fubbik.test`.
    pub fn domain(&self) -> String {
        format!("{}.{}.test", label(&self.process), label(&self.repo))
    }
}

/// A DNS label: lowercase letters, digits and dashes; a name with none of
/// those (`Ünïcödé` keeps `n-c-d`, `—` keeps nothing) falls back to `site`.
fn label(name: &str) -> String {
    let out = raw_label(name);
    if out.is_empty() { "site".into() } else { out }
}

fn raw_label(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// Ports declared by more than one process: port → `repo/process` keys.
pub fn conflicts(sites: &[Site]) -> BTreeMap<u16, Vec<String>> {
    let mut by_port: BTreeMap<u16, Vec<String>> = BTreeMap::new();
    for site in sites {
        by_port.entry(site.port).or_default().push(site.key());
    }
    by_port.retain(|_, keys| keys.len() > 1);
    by_port
}

pub fn caddyfile_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_default()
        .join("caddy/Caddyfile")
}

/// The app's block for these sites (empty when none).
pub fn block(sites: &[Site]) -> String {
    if sites.is_empty() {
        return String::new();
    }
    let mut out = format!("{BEGIN}\n");
    for site in sites {
        out.push_str(&format!(
            "{} {{\n\ttls internal\n\treverse_proxy localhost:{}\n}}\n",
            site.domain(),
            site.port
        ));
    }
    out.push_str(END);
    out.push('\n');
    out
}

/// The Caddyfile with the app's block replaced (or added at the end, or
/// removed when `block` is empty); everything else stays as it was.
pub fn apply(existing: &str, block: &str) -> String {
    let (before, after) = match (existing.find(BEGIN), existing.find(END)) {
        (Some(b), Some(e)) if e > b => {
            let end = e + END.len();
            let end = if existing[end..].starts_with('\n') {
                end + 1
            } else {
                end
            };
            (&existing[..b], &existing[end..])
        }
        _ => (existing, ""),
    };
    let mut out = before.trim_end_matches('\n').to_string();
    if !block.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(block);
    } else if !out.is_empty() {
        out.push('\n');
    }
    let after = after.trim_start_matches('\n');
    if !after.is_empty() {
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push_str(after);
    }
    out
}

/// The domains in the app's block of a Caddyfile.
pub fn managed_domains(caddyfile: &str) -> Vec<String> {
    let (Some(b), Some(e)) = (caddyfile.find(BEGIN), caddyfile.find(END)) else {
        return Vec::new();
    };
    if e < b {
        return Vec::new();
    }
    caddyfile[b..e]
        .lines()
        .filter_map(|l| l.strip_suffix(" {"))
        .map(str::to_string)
        .collect()
}

pub fn write_caddyfile(sites: &[Site]) -> Result<PathBuf, String> {
    let path = caddyfile_path();
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    if !existing.is_empty() {
        std::fs::copy(&path, path.with_extension("fubbik-backup")).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, apply(&existing, &block(sites))).map_err(|e| e.to_string())?;
    Ok(path)
}

/// The domains with no `/etc/hosts` line pointing them at this machine.
pub fn missing_hosts(hosts: &str, domains: &[String]) -> Vec<String> {
    domains
        .iter()
        .filter(|domain| {
            !hosts.lines().any(|line| {
                let line = line.split('#').next().unwrap_or_default();
                let mut fields = line.split_whitespace();
                matches!(fields.next(), Some("127.0.0.1" | "::1"))
                    && fields.any(|name| name == domain.as_str())
            })
        })
        .cloned()
        .collect()
}

pub fn hosts_text() -> String {
    std::fs::read_to_string("/etc/hosts").unwrap_or_default()
}

/// Appends the entries to `/etc/hosts` with sudo (for a terminal tab, where
/// the password prompt shows). Domains are labels the app built, so they
/// need no quoting beyond the single quotes.
pub fn hosts_command(domains: &[String]) -> Option<String> {
    if domains.is_empty() {
        return None;
    }
    let lines: String = domains
        .iter()
        .map(|d| format!("127.0.0.1 {d}\\n"))
        .collect();
    Some(format!(
        "printf '{lines}' | sudo tee -a /etc/hosts >/dev/null && echo 'Added {} to /etc/hosts'",
        domains.join(", ")
    ))
}

pub fn caddy_bin() -> Option<PathBuf> {
    ["/opt/homebrew/bin/caddy", "/usr/local/bin/caddy"]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .or_else(|| {
            std::env::var_os("PATH").and_then(|path| {
                std::env::split_paths(&path)
                    .map(|d| d.join("caddy"))
                    .find(|p| p.is_file())
            })
        })
}

/// Whether a `caddy run` is up (its admin endpoint answers).
pub fn caddy_running() -> bool {
    std::net::TcpStream::connect_timeout(
        &"127.0.0.1:2019".parse().expect("an address"),
        std::time::Duration::from_millis(200),
    )
    .is_ok()
}

/// Runs Caddy on the Caddyfile, reloading when it changes.
pub fn caddy_command() -> Option<String> {
    Some(format!(
        "'{}' run --config '{}' --watch",
        caddy_bin()?.display(),
        caddyfile_path().display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(repo: &str, process: &str, port: u16) -> Site {
        Site {
            repo: repo.into(),
            process: process.into(),
            port,
        }
    }

    #[test]
    fn names_domains_and_finds_conflicts() {
        // Given
        let sites = [
            site("fubbik", "web", 3001),
            site("vpg-platform", "Web App", 3001),
            site("fubbik", "api", 3000),
        ];

        // When
        let conflicts = conflicts(&sites);

        // Then
        assert_eq!(sites[0].domain(), "web.fubbik.test");
        assert_eq!(sites[1].domain(), "web-app.vpg-platform.test");
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[&3001], ["fubbik/web", "vpg-platform/Web App"]);
    }

    #[test]
    fn replaces_only_its_own_block() {
        // Given: the user's own site, then a block from an earlier write
        let mine = "app.mine.test {\n\ttls internal\n\treverse_proxy localhost:8080\n}\n";
        let first = apply(mine, &block(&[site("fubbik", "web", 3001)]));

        // When
        let second = apply(&first, &block(&[site("fubbik", "api", 3000)]));
        let removed = apply(&second, "");

        // Then
        assert!(first.starts_with(mine.trim_end()));
        assert!(
            first.contains("web.fubbik.test {\n\ttls internal\n\treverse_proxy localhost:3001\n}")
        );
        assert!(!second.contains("web.fubbik.test") && second.contains("api.fubbik.test"));
        assert_eq!(second.matches(BEGIN).count(), 1);
        assert_eq!(managed_domains(&second), ["api.fubbik.test"]);
        assert_eq!(removed, mine, "removing leaves exactly what was there");
        assert_eq!(apply("", &block(&[])), "");
    }

    #[test]
    fn finds_missing_hosts_entries() {
        // Given
        let hosts =
            "127.0.0.1 localhost\n127.0.0.1 web.fubbik.test # dev\n# 127.0.0.1 api.fubbik.test\n";
        let domains = vec!["web.fubbik.test".to_string(), "api.fubbik.test".to_string()];

        // When
        let missing = missing_hosts(hosts, &domains);

        // Then
        assert_eq!(missing, ["api.fubbik.test"]);
        assert_eq!(
            hosts_command(&missing).unwrap(),
            "printf '127.0.0.1 api.fubbik.test\\n' | sudo tee -a /etc/hosts >/dev/null && echo 'Added api.fubbik.test to /etc/hosts'"
        );
        assert!(hosts_command(&[]).is_none());
    }

    #[test]
    fn names_that_are_not_ascii_still_make_a_domain() {
        // Given: names with nothing a DNS label can keep, and odd spacing
        let odd = [
            site("Ünïcödé", "web", 1),
            site("app", "—", 2),
            site("--x--", "a  b", 3),
        ];

        // When
        let domains: Vec<String> = odd.iter().map(Site::domain).collect();

        // Then: every label is non-empty and the name is a valid host
        for d in &domains {
            assert!(d.split('.').all(|l| !l.is_empty()), "{d}");
            assert!(
                d.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'),
                "{d}"
            );
        }
        assert_eq!(domains[2], "a-b.x.test");
    }

    #[test]
    fn a_broken_block_is_not_half_replaced() {
        // Given: a BEGIN marker with no END (hand-edited), and CRLF lines
        let broken = format!("site.test {{\r\n}}\r\n{BEGIN}\nweb.app.test {{\n}}\n");
        let mine = "mine.test {\n\treverse_proxy localhost:1\n}\n";

        // When
        let out = apply(&broken, &block(&[site("app", "web", 3000)]));
        let again = apply(
            &apply(mine, &block(&[site("app", "web", 3000)])),
            &block(&[site("app", "web", 3000)]),
        );

        // Then: nothing of the user's is lost, and repeated writes are stable
        assert!(out.contains("site.test"));
        assert_eq!(out.matches(END).count(), 1);
        assert_eq!(again, apply(mine, &block(&[site("app", "web", 3000)])));
    }

    #[test]
    fn hosts_lines_with_tabs_and_many_names_count() {
        // Given
        let hosts =
            "127.0.0.1\tlocalhost a.app.test\tb.app.test\n::1 c.app.test\n10.0.0.1 d.app.test\n";
        let domains: Vec<String> = ["a", "b", "c", "d"]
            .iter()
            .map(|n| format!("{n}.app.test"))
            .collect();

        // When
        let missing = missing_hosts(hosts, &domains);

        // Then: elsewhere-pointed d is missing (it would not reach Caddy)
        assert_eq!(missing, ["d.app.test"]);
    }
}
