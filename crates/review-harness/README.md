# review-harness

Convention checks for AI-written changes. It walks your source tree, applies each rule line by line, prints `file:line [rule] message` for
every violation, and exits non-zero if there is any. It stands alone: no database, no server, no fubbik install needed.

## Install

```bash
cargo install --git https://github.com/Pontusg45/fubbik review-harness
```

From a checkout, use `cargo run -p review-harness -- <args>`.

## Quick start

```bash
review-harness init            # writes ./review-rules.toml with one example rule
review-harness list            # shows every registered check
review-harness run             # exits 1 on any violation
```

| Command                                   | What it does                                                             |
| ----------------------------------------- | ------------------------------------------------------------------------ |
| `review-harness init [--root DIR]`        | Writes a starter `review-rules.toml`. Refuses if the file already exists |
| `review-harness list [--root] [--rules]`  | Lists registered checks with their descriptions                          |
| `review-harness run [--root] [--only a,b] [--rules FILE]` | Runs all checks, or only the named ones. An unknown name is an error |

`--root` is the repository root (default `.`). Rules are read from `<root>/review-rules.toml`. If that file is missing, only compiled checks
run; an explicit `--rules FILE` must exist.

## `review-rules.toml`

One `[[rule]]` table per rule:

```toml
[[rule]]
name = "no-todo-comments"
description = "Unfinished work must be tracked, not left as TODO comments"
pattern = '\bTODO\b'
roots = ["src"]
extensions = [".ts", ".tsx"]
message = "resolve this TODO or move it to the issue tracker"
allow = []
```

| Field         | Meaning                                                                                              |
| ------------- | ---------------------------------------------------------------------------------------------------- |
| `name`        | kebab-case and unique. Used by `--only` and in report output                                         |
| `description` | Shown by `list`                                                                                      |
| `pattern`     | A regex, matched against each line. Use TOML single-quoted strings so backslashes need no escaping   |
| `roots`       | Repo-relative directories to walk                                                                    |
| `extensions`  | File extensions to include, with the dot                                                             |
| `message`     | What to tell the author                                                                              |
| `allow`       | Optional repo-relative path prefixes, no globs, that are exempt from the rule                        |

`allow` is plain prefix matching. End directory prefixes with `/` (`apps/web/src/features/auth/`, not `apps/web/src/features/auth`), or
`.../auth` will also exempt `.../auth-legacy/`.

This repository's own rules are in [`review-rules.toml`](../../review-rules.toml) at the repo root; use them as examples.

## Compiled checks: the `Check` trait

When a regex cannot express a rule (multi-line structure, cross-file logic), write a compiled check. Implement `review_harness::Check`:

```rust
pub trait Check: Send + Sync {
    fn name(&self) -> &str;                 // kebab-case, unique
    fn description(&self) -> &str;
    fn roots(&self) -> Vec<&str>;           // repo-relative directories to walk
    fn extensions(&self) -> Vec<&str>;      // e.g. [".ts", ".tsx"]
    fn check(&self, file: &TextFile) -> Vec<Violation>;  // one file at a time
}
```

Then register it in `src/checks/mod.rs`:

```rust
pub fn all() -> Vec<Box<dyn Check>> {
    vec![Box::new(my_check::MyCheck)]
}
```

Compiled checks and rule-file rules run together and are listed together. Compiled checks need the crate built from source (a fork, or a local
checkout); installing from git gives you only the rule-file mode. See
[Extending fubbik](../../docs/extending.md#add-a-review-check).
