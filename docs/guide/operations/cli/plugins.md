---
tags:
    - guide
    - cli
    - plugins
description: Extend the Rust CLI with external fubbik-* commands
---

# CLI Plugins

The Rust CLI treats an unknown top-level command as an external plugin. A plugin is an executable named `fubbik-<command>` found in one of:

1. `FUBBIK_PLUGIN_PATH`
2. `~/.fubbik/plugins`
3. `~/.local/share/fubbik/plugins`
4. `PATH`

For example, `fubbik audit --changed` executes `fubbik-audit audit --changed`. The command name is deliberately forwarded as the first argument, matching Cargo-style external subcommands.

## Protocol Version 1

Plugins inherit stdin, stdout, stderr, the working directory, and the parent environment. Fubbik also sets:

| Variable | Value |
| --- | --- |
| `FUBBIK_PLUGIN_PROTOCOL` | `1` |
| `FUBBIK_VERSION` | Host CLI version |
| `FUBBIK_URL` | Configured server URL |
| `FUBBIK_OUTPUT` | `human`, `json`, or `quiet` |

Plugins should use the HTTP interface at `FUBBIK_URL`; they should not connect directly to the database or depend on Rust crate internals.

An executable plugin can be written in any language:

```sh
#!/bin/sh

command="$1"
shift

case "$command" in
    audit)
        # Call "$FUBBIK_URL/api/..." and write results to stdout.
        ;;
    *)
        echo "unsupported command: $command" >&2
        exit 2
        ;;
esac
```

Use `fubbik plugin list` to inspect discovered commands and `fubbik plugin doctor` to inspect the active protocol and search path. Built-in commands always take precedence over external plugins.
