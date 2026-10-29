# agents/

Agent material for this repo: skills, slash commands and MCP config.

| Location | What it holds | Edit by hand? |
| --- | --- | --- |
| `agents/` | Skills, commands and MCP config that you write | Yes |
| `.agents/skills/` | Installed third-party skills, managed by `npx skills` and `skills-lock.json` | No |
| `.claude/` | Symlinks only, pointing into `agents/` and `.agents/skills/` | No |

## Generic vs project material

- `agents/skills/` holds generic, shippable skills that make sense in any fork.
- `agents/project/` holds material specific to this repository (for example `agents/project/skills/fubbik-dev`). A fork replaces it with its own.

## Adding a skill

1. Create `agents/skills/<name>/SKILL.md` (generic) or `agents/project/skills/<name>/SKILL.md` (project-specific), with `name` and `description` frontmatter.
2. Run `fk agents link`. It replaces the manual `ln -s` steps: it creates relative symlinks in `.claude/skills/`, repoints stale ones, and prunes dangling links into `agents/`. Use `--dry-run` to preview.

`fk agents link` never touches third-party links into `.agents/skills/`, `.claude/worktrees` or `.claude/settings*.json`, and refuses to replace a real file or directory.

Slash commands live in `agents/commands/`, exposed through the `.claude/commands` symlink. MCP config lives in `agents/mcp/`.

Subagents go in `agents/subagents/` and hooks in `agents/hooks/`. Both are created the first time something goes in them.
