# fubbik for Raycast

Four commands that read the fubbik desktop app (`apps/desktop`) through its MCP server
(`http://127.0.0.1:3100/mcp`, while the app runs):

- **Today**: what needs you, what is in flight and what is next, across every repo (`get_today`)
- **Agents**: live and recently ended coding-agent sessions, with state, branch and idle time (`list_sessions`)
- **Repos**: status, branch, changed files, agents, processes and worktree overlaps (`list_repos`)
- **Capture Note**: a note, todo (shows on Today) or decision, optionally for one repo (`add_note`)

"Open Fubbik" brings the app forward (`open -b dev.fubbik.desktop`).

## Install

This folder is not part of the pnpm workspace; Raycast extensions use npm.

```sh
cd integrations/raycast
npm install
npm run dev      # ray develop: adds the extension to Raycast while it runs
```

The MCP URL is a preference (Raycast → Extensions → fubbik), for a test copy on another port.

`npm run lint` reports one error until `author` in `package.json` is a Raycast Store username; that only matters
for publishing to the store.
