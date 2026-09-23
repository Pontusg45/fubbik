use anyhow::Result;

use crate::output::{self, OutputMode};

const DEFAULT_WEB_URL: &str = "http://localhost:3001";

pub fn run(target: Option<&str>, mode: OutputMode) -> Result<()> {
    let base = std::env::var("FUBBIK_WEB_URL").unwrap_or_else(|_| DEFAULT_WEB_URL.into());
    let url = url_for_target(&base, target);

    match mode {
        OutputMode::Json => output::json(&serde_json::json!({ "url": url }))?,
        OutputMode::Quiet => println!("{url}"),
        OutputMode::Human => {
            if launch(&url) {
                println!("opened {url}");
            } else {
                println!("Open: {url}");
            }
        }
    }
    Ok(())
}

fn url_for_target(base: &str, target: Option<&str>) -> String {
    let route = match target {
        None | Some("dashboard") => "/dashboard",
        Some("graph") => "/graph",
        Some("chunks") => "/chunks",
        Some("requirements") => "/requirements",
        Some("plans") => "/plans",
        Some("import") => "/import",
        Some("settings") => "/settings",
        Some("health") => "/knowledge-health",
        Some("tags") => "/tags",
        Some("docs") => "/docs",
        Some(chunk_id) => return format!("{}/chunks/{chunk_id}", base.trim_end_matches('/')),
    };
    format!("{}{route}", base.trim_end_matches('/'))
}

fn launch(url: &str) -> bool {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).status();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open").arg(url).status();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .status();
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let result: std::io::Result<std::process::ExitStatus> = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "no browser launcher for this platform",
    ));

    result.is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::url_for_target;

    #[test]
    fn known_targets_map_to_web_routes() {
        // Given a configured web application URL
        // When a named page is selected
        let url = url_for_target("https://fubbik.test/", Some("health"));
        // Then the corresponding application route is used
        assert_eq!(url, "https://fubbik.test/knowledge-health");
    }

    #[test]
    fn unknown_targets_are_treated_as_chunk_ids() {
        // Given a target that is not a named page
        // When its URL is built
        let url = url_for_target("http://localhost:3001", Some("chunk-123"));
        // Then it points to the chunk detail page
        assert_eq!(url, "http://localhost:3001/chunks/chunk-123");
    }
}
