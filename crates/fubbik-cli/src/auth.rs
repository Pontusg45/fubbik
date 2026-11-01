use std::collections::BTreeMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::AuthCommand;
use crate::client::Client;
use crate::output::{self, CommandOutput, OutputMode};

fn store_path() -> Result<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|value| !value.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })
        .context("HOME or XDG_CONFIG_HOME is required to save CLI sessions")?;
    Ok(base.join("fubbik/sessions.json"))
}

fn read_store(path: &Path) -> Result<BTreeMap<String, String>> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .with_context(|| format!("invalid session store at {}", path.display())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(error) => Err(error).with_context(|| format!("could not read {}", path.display())),
    }
}

fn write_store(path: &Path, sessions: &BTreeMap<String, String>) -> Result<()> {
    let parent = path.parent().context("session store has no parent")?;
    std::fs::create_dir_all(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
    }
    let temp = parent.join(format!(
        ".sessions-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> Result<()> {
        use std::io::Write;
        let mut file = options.open(&temp)?;
        serde_json::to_writer(&mut file, sessions)?;
        file.flush()?;
        file.sync_all()?;
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
}

fn key(base: &str) -> String {
    base.trim_end_matches('/').to_owned()
}

pub fn token_for(base: &str) -> Result<Option<String>> {
    if let Ok(token) = std::env::var("FUBBIK_SESSION") {
        if !token.is_empty() {
            return Ok(Some(token));
        }
    }
    Ok(read_store(&store_path()?)?.get(&key(base)).cloned())
}

pub async fn run(base: &str, command: AuthCommand, mode: OutputMode) -> Result<()> {
    match command {
        AuthCommand::Login {
            email,
            password_stdin,
        } => {
            let url = reqwest::Url::parse(base).context("invalid server URL")?;
            let local = matches!(
                url.host_str(),
                Some("localhost" | "127.0.0.1" | "::1" | "[::1]")
            );
            if url.scheme() != "https" && !(url.scheme() == "http" && local) {
                bail!("login requires HTTPS except for localhost");
            }
            let password = if password_stdin {
                let mut password = String::new();
                io::stdin().read_to_string(&mut password)?;
                password.trim_end_matches(['\r', '\n']).to_owned()
            } else {
                rpassword::prompt_password("Password: ")?
            };
            let client = Client::new(base);
            let signed_in = client.sign_in(&email, &password).await?;
            let path = store_path()?;
            let mut sessions = read_store(&path)?;
            sessions.insert(key(base), signed_in.token);
            write_store(&path, &sessions)?;
            output::render(
                mode,
                CommandOutput {
                    data: signed_in.user.clone(),
                    human: format!(
                        "Signed in as {}",
                        signed_in.user["email"].as_str().unwrap_or(&email)
                    ),
                    quiet: vec![
                        signed_in.user["email"]
                            .as_str()
                            .unwrap_or(&email)
                            .to_owned(),
                    ],
                },
            )?;
        }
        AuthCommand::Logout => {
            let path = store_path()?;
            let mut sessions = read_store(&path)?;
            let mut tokens = std::collections::BTreeSet::new();
            if let Some(token) = sessions.get(&key(base)).cloned() {
                tokens.insert(token);
            }
            if let Ok(token) = std::env::var("FUBBIK_SESSION")
                && !token.is_empty()
            {
                tokens.insert(token);
            }
            for token in tokens {
                Client::with_token(base, Some(&token))?.sign_out().await?;
            }
            if sessions.remove(&key(base)).is_some() {
                write_store(&path, &sessions)?;
            }
            output::render(
                mode,
                CommandOutput {
                    data: serde_json::json!({"signedOut": true}),
                    human: "Signed out".into(),
                    quiet: vec![],
                },
            )?;
        }
        AuthCommand::Whoami => {
            let client = Client::authenticated(base)?;
            let session = client.current_session().await?.context("not signed in")?;
            let user = session["user"].clone();
            output::render(
                mode,
                CommandOutput {
                    human: user["email"].as_str().unwrap_or("Unknown user").to_owned(),
                    quiet: user["email"]
                        .as_str()
                        .map(str::to_owned)
                        .into_iter()
                        .collect(),
                    data: user,
                },
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_store_round_trips_and_is_private() {
        // Given a session for one server
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fubbik/sessions.json");
        let mut sessions = BTreeMap::new();
        sessions.insert("https://example.com".into(), "secret".into());
        // When the store is written and read
        write_store(&path, &sessions).unwrap();
        let actual = read_store(&path).unwrap();
        // Then the token survives without becoming public
        assert_eq!(actual, sessions);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
