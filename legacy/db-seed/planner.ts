import type { ScenarioName } from "./context";

export interface SeedModuleDescriptor {
    name: string;
    deps: string[];
    scenarios: ScenarioName[];
}

export interface SeedSelection {
    scenario: ScenarioName;
    only?: ReadonlySet<string>;
    skip?: ReadonlySet<string>;
}

export function planModules(registry: readonly SeedModuleDescriptor[], selection: SeedSelection): string[] {
    const modules = new Map<string, SeedModuleDescriptor>();
    for (const module of registry) {
        if (modules.has(module.name)) throw new Error(`Duplicate seed module: ${module.name}`);
        modules.set(module.name, module);
    }
    for (const module of registry) {
        for (const dependency of module.deps) {
            if (!modules.has(dependency)) {
                throw new Error(`Seed module ${module.name} has unknown dependency: ${dependency}`);
            }
        }
    }

    const selected = new Set<string>();
    const includeWithDependencies = (name: string) => {
        const module = modules.get(name);
        if (!module) throw new Error(`Unknown seed module: ${name}`);
        if (selected.has(name)) return;
        selected.add(name);
        for (const dependency of module.deps) includeWithDependencies(dependency);
    };

    if (selection.only) {
        for (const name of selection.only) {
            const module = modules.get(name);
            if (!module) throw new Error(`Unknown seed module: ${name}`);
            if (!module.scenarios.includes(selection.scenario)) {
                throw new Error(`Seed module ${name} is not part of scenario ${selection.scenario}`);
            }
            includeWithDependencies(name);
        }
    } else {
        for (const module of registry) {
            if (module.scenarios.includes(selection.scenario)) selected.add(module.name);
        }
    }

    if (selection.skip) {
        for (const name of selection.skip) {
            if (!modules.has(name)) throw new Error(`Unknown seed module: ${name}`);
            selected.delete(name);
        }
    }

    for (const name of selected) {
        const module = modules.get(name)!;
        for (const dependency of module.deps) {
            if (!selected.has(dependency)) {
                throw new Error(`Cannot skip ${dependency}; required by ${name}`);
            }
        }
    }

    const ordered: string[] = [];
    const visiting = new Set<string>();
    const visited = new Set<string>();
    const visit = (name: string) => {
        if (visited.has(name)) return;
        if (visiting.has(name)) {
            const cycle = [...visiting].filter(candidate => !visited.has(candidate)).sort();
            throw new Error(`Seed module dependency cycle: ${cycle.join(", ")}`);
        }
        visiting.add(name);
        const module = modules.get(name)!;
        for (const dependency of [...module.deps].sort()) {
            if (selected.has(dependency)) visit(dependency);
        }
        visiting.delete(name);
        visited.add(name);
        ordered.push(name);
    };

    for (const name of [...selected].sort()) visit(name);
    return ordered;
}
