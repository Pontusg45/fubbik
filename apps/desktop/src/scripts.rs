//! A repo's runnable scripts: `just` recipes, `package.json` scripts and
//! Makefile targets, read from the files (no tool needs to be installed).

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    /// "just", "pnpm"/"npm"/…, or "make".
    pub tool: String,
    pub name: String,
    pub command: String,
}

/// Recipe names from a justfile: lines starting a recipe (`name args:`),
/// skipping private (`_x`) recipes, settings, variables and comments.
pub fn just_recipes(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.starts_with([' ', '\t', '#', '@', '[']) && !line.trim().is_empty())
        .filter_map(|line| {
            let (head, _) = line.split_once(':')?;
            if line.contains(":=")
                || head.starts_with("set ")
                || head.starts_with("export ")
                || head.starts_with("import")
            {
                return None;
            }
            let name = head.split_whitespace().next()?;
            let ok = !name.starts_with('_')
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
            ok.then(|| name.to_string())
        })
        .collect()
}

/// Target names from a Makefile, without special (`.PHONY`) or pattern
/// (`%.o`) targets and variable assignments.
pub fn make_targets(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.starts_with(['\t', ' ', '#', '.']))
        .filter_map(|line| {
            let (head, rest) = line.split_once(':')?;
            if rest.starts_with('=') || head.contains(['%', '$', '=']) {
                return None;
            }
            let name = head.trim();
            (!name.is_empty() && !name.contains(' ')).then(|| name.to_string())
        })
        .collect()
}

fn package_manager(repo: &Path) -> &'static str {
    let has = |name: &str| repo.join(name).exists();
    if has("pnpm-lock.yaml") {
        "pnpm"
    } else if has("bun.lockb") || has("bun.lock") {
        "bun"
    } else if has("yarn.lock") {
        "yarn"
    } else {
        "npm"
    }
}

pub fn detect(repo: &Path) -> Vec<Script> {
    let mut scripts = Vec::new();
    let justfile = ["justfile", "Justfile", ".justfile"]
        .iter()
        .find_map(|name| std::fs::read_to_string(repo.join(name)).ok());
    if let Some(text) = justfile {
        scripts.extend(just_recipes(&text).into_iter().map(|name| Script {
            tool: "just".into(),
            command: format!("just {name}"),
            name,
        }));
    }
    if let Ok(text) = std::fs::read_to_string(repo.join("package.json"))
        && let Ok(json) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(map) = json["scripts"].as_object()
    {
        let pm = package_manager(repo);
        scripts.extend(map.keys().map(|name| Script {
            tool: pm.into(),
            command: format!("{pm} run {name}"),
            name: name.clone(),
        }));
    }
    if let Some(text) = ["Makefile", "makefile", "GNUmakefile"]
        .iter()
        .find_map(|name| std::fs::read_to_string(repo.join(name)).ok())
    {
        scripts.extend(make_targets(&text).into_iter().map(|name| Script {
            tool: "make".into(),
            command: format!("make {name}"),
            name,
        }));
    }
    scripts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_just_recipes() {
        // Given
        let text = "set shell := [\"zsh\"]\nexport FOO := \"1\"\n# Run it\ndev:\n    cargo run\n\nbuild target='x': dev\n    echo\n_private:\n    true\n[group('x')]\ntest *args:\n    cargo test\n";

        // When
        let recipes = just_recipes(text);

        // Then
        assert_eq!(recipes, ["dev", "build", "test"]);
    }

    #[test]
    fn reads_make_targets() {
        // Given
        let text = ".PHONY: all test\nCC := cc\nall: build\n\tcc main.c\n%.o: %.c\n\tcc -c $<\ntest:\n\t./run\n";

        // When
        let targets = make_targets(text);

        // Then
        assert_eq!(targets, ["all", "test"]);
    }

    #[test]
    fn detects_all_three_kinds() {
        // Given
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("justfile"), "dev:\n    run\n").unwrap();
        std::fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"lint":"oxlint"}}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("Makefile"), "clean:\n\trm -rf out\n").unwrap();

        // When
        let commands: Vec<String> = detect(dir.path()).into_iter().map(|s| s.command).collect();

        // Then
        assert_eq!(commands, ["just dev", "pnpm run lint", "make clean"]);
    }
}
