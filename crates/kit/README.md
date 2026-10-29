# fk

`fk` is a small terminal CLI for the git and dev chores an AI agent runs most. Each command is safe by default: it refuses rather than guesses.
It stands alone. You do not need the rest of fubbik, a database, or a fork.

## Install

```bash
cargo install --git https://github.com/Pontusg45/fubbik kit
```

The package is called `kit`; the binary it installs is `fk`. From a checkout, `cargo run -p kit -- <args>` works too.

## Global flag

`--json` goes before or after the subcommand and prints machine-readable JSON where a command has a result to report (`sync`, `push`,
`port kill`, `agents link`). `fk branch` prints nothing with `--json`; check the exit code. Errors go to stderr as `error: ...` and exit with
status 1.

## Commands

### `fk sync [--dry-run]`

Fetches, then fast-forwards the current branch to its upstream. It never rebases and never merges.

- Refuses when there is no upstream configured.
- Refuses when the branch has diverged from its upstream; resolve that by hand.
- `--dry-run` reports how many commits it would fast-forward and changes nothing.

### `fk push [--yes]`

Lists the unpushed commits, then asks `Push? [y/N]` before pushing.

- Without a terminal on stdin it refuses unless you pass `--yes`. An agent cannot push by accident.
- Anything other than `y`, `Y` or `yes` aborts.
- With `--json` the plan (branch, upstream, commits) is printed; the confirmation still applies.

### `fk branch <name> [--from <base>]`

Creates a branch and switches to it (`git switch -c`).

- Refuses when the working tree has uncommitted changes.
- `--from` names the start point; without it the branch starts at `HEAD`.

### `fk port kill <port>... [--dry-run]`

Kills whatever is listening on each TCP port. It needs `lsof`.

- Only processes listening on the port are matched.
- A free port is reported as free and is not an error.
- Kills with `kill -9`. Use `--dry-run` to see the PIDs first.
- Several ports can be given; a failure on one is reported and the command exits non-zero after trying the rest.

### `fk agents link [--dry-run]`

Unix only. On other platforms it fails with `fk agents link requires a Unix-like OS`.

Exposes agent material to Claude Code with relative symlinks, run from anywhere inside the repo (it uses the git toplevel).

- Each directory containing a `SKILL.md` under `agents/skills/` and `agents/project/skills/` is linked into `.claude/skills/<name>`.
- `agents/commands` is linked as `.claude/commands`, when that directory exists.
- Only `.claude/skills/<name>` entries and the `.claude/commands` link are managed. Other skill links and settings under `.claude/` are left
  alone. A symlink that is dangling and points into `../../agents/` is pruned.
- Refuses to replace a real file or directory at a link path, and refuses when the same skill name appears in both source directories.
  It plans everything first, so a refusal changes nothing.
- Re-running is safe: links that already point at the right place are reported as `AlreadyLinked`.

## Library

The commands live in the `kit` library (`crates/kit/src`), one module per command, if you want to reuse them from Rust. To add a command,
see [Extending fubbik](../../docs/extending.md#add-an-fk-subcommand).
