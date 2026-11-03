//! `fubbik://` links and the MCP `show` tool: open the app at a tab, for
//! every repo or one. `fubbik://today`, `fubbik://agents`,
//! `fubbik://repo/fubbik`, `fubbik://repo/fubbik/changes`.

use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// A tab name as the app labels it, any case (`today`, `changes`,
    /// `ai sessions`); None: the repo's last tab, or Home.
    pub tab: Option<String>,
    pub repo: Option<String>,
}

/// `fubbik://today` → Today; `fubbik://repo/<name>[/<tab>]` → that repo.
/// Percent-escapes in names are decoded (`ai%20sessions`).
pub fn parse(url: &str) -> Option<Link> {
    let rest = url.strip_prefix("fubbik://")?;
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    let parts: Vec<String> = rest
        .split('/')
        .filter(|p| !p.is_empty())
        .map(decode)
        .collect();
    match parts.as_slice() {
        [] => Some(Link {
            tab: None,
            repo: None,
        }),
        [first, name, tab @ ..] if first == "repo" => Some(Link {
            repo: Some(name.clone()),
            tab: tab.first().cloned(),
        }),
        [first] if first == "repo" => None,
        [tab] => Some(Link {
            tab: Some(tab.clone()),
            repo: None,
        }),
        _ => None,
    }
}

/// Percent-decoding on bytes: a `%` not followed by two hex digits stays as
/// it is (never a panic on a link from outside).
fn decode(part: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        (b as char).to_digit(16).map(|d| d as u8)
    }
    let bytes = part.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match (
            bytes[i],
            bytes.get(i + 1).copied().and_then(hex),
            bytes.get(i + 2).copied().and_then(hex),
        ) {
            (b'%', Some(high), Some(low)) => {
                out.push(high << 4 | low);
                i += 3;
            }
            (byte, _, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

type Channel = (async_channel::Sender<Link>, async_channel::Receiver<Link>);

fn channel() -> &'static std::sync::Mutex<Channel> {
    static CHANNEL: OnceLock<std::sync::Mutex<Channel>> = OnceLock::new();
    CHANNEL.get_or_init(|| std::sync::Mutex::new(async_channel::unbounded()))
}

pub fn send(link: Link) {
    let _ = channel().lock().unwrap().0.try_send(link);
}

pub fn receiver() -> async_channel::Receiver<Link> {
    channel().lock().unwrap().1.clone()
}

/// A fresh channel, so a test's workspace is the only listener (an earlier
/// test's app can leave a waiting receiver that would be woken instead).
#[cfg(test)]
pub fn reset() {
    *channel().lock().unwrap() = async_channel::unbounded();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tabs_and_repos() {
        // Given / When / Then
        let link = |tab: Option<&str>, repo: Option<&str>| Link {
            tab: tab.map(str::to_string),
            repo: repo.map(str::to_string),
        };
        assert_eq!(parse("fubbik://today"), Some(link(Some("today"), None)));
        assert_eq!(parse("fubbik://"), Some(link(None, None)));
        assert_eq!(
            parse("fubbik://repo/fubbik"),
            Some(link(None, Some("fubbik")))
        );
        assert_eq!(
            parse("fubbik://repo/vpg-bot-v2/ai%20sessions?x=1"),
            Some(link(Some("ai sessions"), Some("vpg-bot-v2")))
        );
        assert_eq!(parse("fubbik://repo"), None);
        assert_eq!(parse("https://today"), None);
        assert_eq!(parse("fubbik://a/b"), None);
    }

    #[test]
    fn odd_escapes_never_panic() {
        // Given: escapes cut short, not hex, or next to multi-byte letters
        let urls = [
            "fubbik://repo/%",
            "fubbik://repo/a%2",
            "fubbik://repo/%zz/changes",
            "fubbik://repo/%aé",
            "fubbik://repo/é%41",
            "fubbik://repo/%E2%9C%93",
            "fubbik://today/",
            "fubbik:///today",
        ];

        // When
        let parsed: Vec<Option<Link>> = urls.iter().map(|u| parse(u)).collect();

        // Then
        assert_eq!(parsed[0].as_ref().unwrap().repo.as_deref(), Some("%"));
        assert_eq!(parsed[2].as_ref().unwrap().repo.as_deref(), Some("%zz"));
        assert_eq!(parsed[3].as_ref().unwrap().repo.as_deref(), Some("%aé"));
        assert_eq!(parsed[4].as_ref().unwrap().repo.as_deref(), Some("éA"));
        assert_eq!(parsed[5].as_ref().unwrap().repo.as_deref(), Some("✓"));
        assert_eq!(parsed[6].as_ref().unwrap().tab.as_deref(), Some("today"));
        assert_eq!(parsed[7].as_ref().unwrap().tab.as_deref(), Some("today"));
    }
}
