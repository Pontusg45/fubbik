# agents/

Agent material for this repo: skills, slash commands and MCP config.

| Location | What it holds | Edit by hand? |
| --- | --- | --- |
| `agents/` | Skills, commands and MCP config that you write | Yes |
| `.agents/skills/` | Installed third-party skills, managed by `npx skills` and `skills-lock.json` | No |
| `.claude/` | Symlinks only, pointing into `agents/` and `.agents/skills/` | No |

## Adding a skill

1. Create `agents/skills/<name>/SKILL.md` (with `name` and `description` frontmatter).
2. Link it into Claude Code: `ln -s ../../agents/skills/<name> .claude/skills/<name>`

Slash commands live in `agents/commands/`, exposed through the `.claude/commands` symlink. MCP config lives in `agents/mcp/`.

Subagents go in `agents/subagents/` and hooks in `agents/hooks/`. Both are created the first time something goes in them.
