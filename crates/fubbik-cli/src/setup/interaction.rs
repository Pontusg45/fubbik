use super::*;

pub(super) fn git_remote() -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
}

pub(super) fn project_name(directory: &Path, remote: Option<&str>) -> String {
    remote
        .and_then(|remote| remote.trim_end_matches(".git").rsplit(['/', ':']).next())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| directory.file_name()?.to_str().map(str::to_owned))
        .unwrap_or_else(|| "my-project".into())
}

pub(super) fn prompt_name(default: &str) -> Result<String> {
    print!("No git remote detected. Space name [{default}]: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    let value = value.trim();
    Ok(if value.is_empty() {
        default.into()
    } else {
        value.into()
    })
}

pub(super) fn confirm(question: &str) -> Result<bool> {
    print!("{question} [y/N] ");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}
