use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeedScenario {
    Minimal,
    Demo,
    Extended,
}

impl SeedScenario {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Demo => "demo",
            Self::Extended => "extended",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResetPolicy {
    Auto,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeedOptions {
    pub scenario: SeedScenario,
    pub only: Option<BTreeSet<String>>,
    pub skip: BTreeSet<String>,
    pub reset: ResetPolicy,
}

impl Default for SeedOptions {
    fn default() -> Self {
        Self {
            scenario: SeedScenario::Demo,
            only: None,
            skip: BTreeSet::new(),
            reset: ResetPolicy::Auto,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SeedModule {
    pub name: &'static str,
    pub dependencies: &'static [&'static str],
    pub scenarios: &'static [SeedScenario],
}

const EVERY_SCENARIO: &[SeedScenario] = &[
    SeedScenario::Minimal,
    SeedScenario::Demo,
    SeedScenario::Extended,
];
const DEMO_AND_EXTENDED: &[SeedScenario] = &[SeedScenario::Demo, SeedScenario::Extended];

pub const MODULES: &[SeedModule] = &[
    SeedModule {
        name: "core",
        dependencies: &[],
        scenarios: EVERY_SCENARIO,
    },
    SeedModule {
        name: "codebases",
        dependencies: &["core"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "tags",
        dependencies: &["core"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "chunks",
        dependencies: &["core", "codebases", "tags"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "connections",
        dependencies: &["chunks"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "file-links",
        dependencies: &["chunks"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "use-cases",
        dependencies: &["core", "codebases"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "requirements",
        dependencies: &["core", "codebases", "use-cases", "chunks"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "plans",
        dependencies: &["core", "codebases", "requirements", "chunks"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "agent-coordination",
        dependencies: &["plans"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "documents",
        dependencies: &["core", "codebases"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "vocabulary",
        dependencies: &["core", "codebases"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "workspaces",
        dependencies: &["core", "codebases"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "collections",
        dependencies: &["core", "chunks"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "matrices",
        dependencies: &["core", "codebases", "requirements"],
        scenarios: DEMO_AND_EXTENDED,
    },
    SeedModule {
        name: "self-documenting",
        dependencies: &["core", "codebases", "tags"],
        scenarios: EVERY_SCENARIO,
    },
];

/// Produces a deterministic topological order matching the legacy scenario
/// selection rules. The caller can run this before opening a transaction.
pub fn plan_modules(
    registry: &[SeedModule],
    options: &SeedOptions,
) -> Result<Vec<&'static str>, String> {
    let mut modules = BTreeMap::new();
    for module in registry {
        if modules.insert(module.name, module).is_some() {
            return Err(format!("Duplicate seed module: {}", module.name));
        }
    }
    for module in registry {
        for dependency in module.dependencies {
            if !modules.contains_key(dependency) {
                return Err(format!(
                    "Seed module {} has unknown dependency: {dependency}",
                    module.name
                ));
            }
        }
    }

    let mut selected = BTreeSet::new();
    if let Some(only) = &options.only {
        for name in only {
            let module = modules
                .get(name.as_str())
                .ok_or_else(|| format!("Unknown seed module: {name}"))?;
            if !module.scenarios.contains(&options.scenario) {
                return Err(format!(
                    "Seed module {name} is not part of scenario {}",
                    options.scenario.as_str()
                ));
            }
            include_dependencies(name, &modules, &mut selected)?;
        }
    } else {
        // Scenario modules may depend on shared modules outside that scenario
        // (minimal's self-documenting module needs codebases and tags).
        for module in registry
            .iter()
            .filter(|module| module.scenarios.contains(&options.scenario))
        {
            include_dependencies(module.name, &modules, &mut selected)?;
        }
    }

    for name in &options.skip {
        if !modules.contains_key(name.as_str()) {
            return Err(format!("Unknown seed module: {name}"));
        }
        selected.remove(name.as_str());
    }
    for name in &selected {
        for dependency in modules[name].dependencies {
            if !selected.contains(dependency) {
                return Err(format!("Cannot skip {dependency}; required by {name}"));
            }
        }
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    for name in &selected {
        visit(
            name,
            &modules,
            &selected,
            &mut visiting,
            &mut visited,
            &mut ordered,
        )?;
    }
    Ok(ordered)
}

fn include_dependencies(
    name: &str,
    modules: &BTreeMap<&str, &SeedModule>,
    selected: &mut BTreeSet<&'static str>,
) -> Result<(), String> {
    let module = modules
        .get(name)
        .ok_or_else(|| format!("Unknown seed module: {name}"))?;
    if selected.insert(module.name) {
        for dependency in module.dependencies {
            include_dependencies(dependency, modules, selected)?;
        }
    }
    Ok(())
}

fn visit(
    name: &'static str,
    modules: &BTreeMap<&str, &SeedModule>,
    selected: &BTreeSet<&str>,
    visiting: &mut BTreeSet<&str>,
    visited: &mut BTreeSet<&str>,
    ordered: &mut Vec<&'static str>,
) -> Result<(), String> {
    if visited.contains(name) {
        return Ok(());
    }
    if !visiting.insert(name) {
        return Err(format!(
            "Seed module dependency cycle: {}",
            visiting.iter().copied().collect::<Vec<_>>().join(", ")
        ));
    }
    for dependency in modules[name].dependencies {
        if selected.contains(dependency) {
            visit(dependency, modules, selected, visiting, visited, ordered)?;
        }
    }
    visiting.remove(name);
    visited.insert(name);
    ordered.push(name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[SeedModule] = &[
        SeedModule {
            name: "core",
            dependencies: &[],
            scenarios: EVERY_SCENARIO,
        },
        SeedModule {
            name: "tags",
            dependencies: &["core"],
            scenarios: DEMO_AND_EXTENDED,
        },
        SeedModule {
            name: "chunks",
            dependencies: &["core", "tags"],
            scenarios: DEMO_AND_EXTENDED,
        },
        SeedModule {
            name: "plans",
            dependencies: &["chunks"],
            scenarios: DEMO_AND_EXTENDED,
        },
    ];

    #[test]
    fn only_selection_includes_transitive_dependencies() {
        // Given a request for plans in the demo scenario
        let options = SeedOptions {
            only: Some(BTreeSet::from(["plans".into()])),
            ..SeedOptions::default()
        };

        // When the selected modules are planned
        let modules = plan_modules(SAMPLE, &options).unwrap();

        // Then all required modules precede plans
        assert_eq!(modules, ["core", "tags", "chunks", "plans"]);
    }

    #[test]
    fn skipping_a_required_dependency_is_rejected() {
        // Given a demo run that skips tags while retaining chunks
        let options = SeedOptions {
            skip: BTreeSet::from(["tags".into()]),
            ..SeedOptions::default()
        };

        // When the run is planned
        let error = plan_modules(SAMPLE, &options).unwrap_err();

        // Then the dependency conflict is named
        assert_eq!(error, "Cannot skip tags; required by chunks");
    }

    #[test]
    fn sorting_does_not_depend_on_registry_order() {
        // Given a reversed registry
        let reversed = SAMPLE.iter().copied().rev().collect::<Vec<_>>();

        // When the demo modules are planned
        let modules = plan_modules(&reversed, &SeedOptions::default()).unwrap();

        // Then the dependency order remains deterministic
        assert_eq!(modules, ["core", "tags", "chunks", "plans"]);
    }

    #[test]
    fn cycles_name_the_participating_modules() {
        // Given a cycle between two demo modules
        let cyclic = [
            SeedModule {
                name: "one",
                dependencies: &["two"],
                scenarios: DEMO_AND_EXTENDED,
            },
            SeedModule {
                name: "two",
                dependencies: &["one"],
                scenarios: DEMO_AND_EXTENDED,
            },
        ];

        // When the modules are planned
        let error = plan_modules(&cyclic, &SeedOptions::default()).unwrap_err();

        // Then the cycle is rejected with its members
        assert!(error.contains("Seed module dependency cycle:"));
        assert!(error.contains("one"));
        assert!(error.contains("two"));
    }

    #[test]
    fn minimal_scenario_closes_self_description_dependencies() {
        // Given the complete module registry and minimal scenario
        let options = SeedOptions {
            scenario: SeedScenario::Minimal,
            ..SeedOptions::default()
        };

        // When the selected modules are planned
        let modules = plan_modules(MODULES, &options).unwrap();

        // Then codebases and tags are included for the self-description
        assert_eq!(modules, ["core", "codebases", "tags", "self-documenting"]);
    }
}
