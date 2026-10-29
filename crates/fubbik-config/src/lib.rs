//! Project-level configuration read from `fubbik.toml`.
//!
//! The file is optional and committed with a fork. Its `[features]` table
//! switches whole route groups on or off at server start; every feature is on
//! when the file, the table or a key is absent. Personal settings (secrets,
//! URLs, ports) live in `.env`, not here.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A switchable route group. Everything not listed here is `core` and is
/// always mounted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Feature {
    Plans,
    Requirements,
    Matrices,
    Coordination,
    LearningPaths,
    Overlays,
    Ai,
}

impl Feature {
    /// Every feature, in the order `enabled_keys` reports them.
    pub const ALL: [Feature; 7] = [
        Feature::Plans,
        Feature::Requirements,
        Feature::Matrices,
        Feature::Coordination,
        Feature::LearningPaths,
        Feature::Overlays,
        Feature::Ai,
    ];

    /// The key used in `fubbik.toml`'s `[features]` table and in
    /// `GET /api/capabilities`.
    pub fn key(self) -> &'static str {
        match self {
            Feature::Plans => "plans",
            Feature::Requirements => "requirements",
            Feature::Matrices => "matrices",
            Feature::Coordination => "coordination",
            Feature::LearningPaths => "learning_paths",
            Feature::Overlays => "overlays",
            Feature::Ai => "ai",
        }
    }

    /// The feature this one cannot run without, if any.
    pub fn requires(self) -> Option<Feature> {
        match self {
            Feature::Matrices => Some(Feature::Requirements),
            Feature::Coordination => Some(Feature::Plans),
            _ => None,
        }
    }

    fn from_key(key: &str) -> Option<Feature> {
        Feature::ALL.into_iter().find(|f| f.key() == key)
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// Which features are switched on. `Default` turns every feature on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Features {
    enabled: [bool; Feature::ALL.len()],
}

impl Default for Features {
    fn default() -> Self {
        Self {
            enabled: [true; Feature::ALL.len()],
        }
    }
}

impl Features {
    pub fn enabled(&self, feature: Feature) -> bool {
        self.enabled[feature.index()]
    }

    /// Keys of the enabled features, in `Feature::ALL` order.
    pub fn enabled_keys(&self) -> Vec<&'static str> {
        Feature::ALL
            .into_iter()
            .filter(|f| self.enabled(*f))
            .map(Feature::key)
            .collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("unknown feature \"{key}\"; valid features: {valid}", valid = valid_keys())]
    UnknownFeature { key: String },
    #[error("feature \"{feature}\" requires \"{requires}\"")]
    MissingDependency {
        feature: &'static str,
        requires: &'static str,
    },
    #[error("invalid fubbik.toml: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("cannot read {path}: {source}", path = path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
}

fn valid_keys() -> String {
    Feature::ALL.map(Feature::key).join(", ")
}

/// The parsed contents of `fubbik.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    pub features: Features,
}

/// The file's raw shape. Other top-level tables are ignored, so later
/// sections can be added without breaking older binaries.
#[derive(serde::Deserialize)]
struct RawConfig {
    #[serde(default)]
    features: BTreeMap<String, bool>,
}

impl Config {
    pub fn from_toml_str(source: &str) -> Result<Config, ConfigError> {
        let raw: RawConfig = toml::from_str(source)?;

        let mut features = Features::default();
        for (key, on) in raw.features {
            let feature = Feature::from_key(&key).ok_or(ConfigError::UnknownFeature { key })?;
            features.enabled[feature.index()] = on;
        }

        for feature in Feature::ALL {
            if let Some(required) = feature.requires()
                && features.enabled(feature)
                && !features.enabled(required)
            {
                return Err(ConfigError::MissingDependency {
                    feature: feature.key(),
                    requires: required.key(),
                });
            }
        }

        Ok(Config { features })
    }

    /// Reads the file named by `FUBBIK_CONFIG` if set, otherwise
    /// `./fubbik.toml` if it exists, otherwise returns the default (all
    /// features on).
    pub fn load() -> Result<Config, ConfigError> {
        if let Some(path) = std::env::var_os("FUBBIK_CONFIG") {
            return Self::from_path(Path::new(&path));
        }
        let local = Path::new("fubbik.toml");
        if local.is_file() {
            return Self::from_path(local);
        }
        Ok(Config::default())
    }

    fn from_path(path: &Path) -> Result<Config, ConfigError> {
        let source = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_toml_str(&source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// `load()` reads the process environment and current directory, both of
    /// which are process-global. Every test that touches them holds this lock.
    static PROCESS_STATE: Mutex<()> = Mutex::new(());

    fn all_enabled(features: &Features) -> bool {
        Feature::ALL.iter().all(|f| features.enabled(*f))
    }

    #[test]
    fn missing_file_enables_everything() {
        let _guard = PROCESS_STATE.lock().unwrap_or_else(|e| e.into_inner());
        // Given
        let dir = tempfile::tempdir().unwrap();
        let previous = std::env::current_dir().unwrap();
        // SAFETY: serialised by PROCESS_STATE; no other thread reads the env.
        unsafe { std::env::remove_var("FUBBIK_CONFIG") };
        std::env::set_current_dir(dir.path()).unwrap();

        // When
        let result = Config::load();
        std::env::set_current_dir(previous).unwrap();

        // Then
        let config = result.unwrap();
        assert!(all_enabled(&config.features));
    }

    #[test]
    fn empty_features_table_enables_everything() {
        // Given
        let source = "[features]\n";

        // When
        let config = Config::from_toml_str(source).unwrap();

        // Then
        assert!(all_enabled(&config.features));
        assert_eq!(
            config.features.enabled_keys(),
            vec![
                "plans",
                "requirements",
                "matrices",
                "coordination",
                "learning_paths",
                "overlays",
                "ai"
            ]
        );
    }

    #[test]
    fn false_disables_one_feature() {
        // Given
        let source = "[features]\nai = false\n";

        // When
        let config = Config::from_toml_str(source).unwrap();

        // Then
        assert!(!config.features.enabled(Feature::Ai));
        for feature in Feature::ALL.iter().filter(|f| **f != Feature::Ai) {
            assert!(
                config.features.enabled(*feature),
                "{feature:?} should be on"
            );
        }
    }

    #[test]
    fn unknown_feature_key_is_rejected() {
        // Given
        let source = "[features]\nplan = false\n";

        // When
        let error = Config::from_toml_str(source).unwrap_err();

        // Then
        assert_eq!(
            error.to_string(),
            "unknown feature \"plan\"; valid features: plans, requirements, matrices, \
             coordination, learning_paths, overlays, ai"
        );
    }

    #[test]
    fn dependency_is_enforced() {
        // Given
        let source = "[features]\nrequirements = false\n";

        // When
        let error = Config::from_toml_str(source).unwrap_err();

        // Then
        assert_eq!(
            error.to_string(),
            "feature \"matrices\" requires \"requirements\""
        );
    }

    #[test]
    fn example_file_lists_every_feature_turned_on() {
        // Given
        let source = include_str!("../../../fubbik.example.toml");

        // When
        let config = Config::from_toml_str(source).unwrap();
        let raw: RawConfig = toml::from_str(source).unwrap();

        // Then
        assert!(all_enabled(&config.features));
        let listed: Vec<&str> = raw.features.keys().map(String::as_str).collect();
        let mut expected: Vec<&str> = Feature::ALL.map(Feature::key).to_vec();
        expected.sort_unstable();
        assert_eq!(listed, expected);
    }

    #[test]
    fn fubbik_config_env_points_at_a_file() {
        let _guard = PROCESS_STATE.lock().unwrap_or_else(|e| e.into_inner());
        // Given
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("custom.toml");
        std::fs::write(&path, "[features]\noverlays = false\n").unwrap();
        // SAFETY: serialised by PROCESS_STATE; no other thread reads the env.
        unsafe { std::env::set_var("FUBBIK_CONFIG", &path) };

        // When
        let result = Config::load();
        // SAFETY: as above.
        unsafe { std::env::remove_var("FUBBIK_CONFIG") };

        // Then
        let config = result.unwrap();
        assert!(!config.features.enabled(Feature::Overlays));
        assert!(config.features.enabled(Feature::Ai));
    }
}
