# Feature switches (fubbik.toml)

Reference for CLAUDE.md. Read when adding a feature group, route, or background job that a fork might disable.

A fork can turn whole feature groups off with an optional, committed `fubbik.toml` (template: `fubbik.example.toml`; loader:
`crates/fubbik-config`). Personal settings (secrets, URLs, ports) stay in `.env`.

```toml
[features]
plans = true
requirements = true
matrices = true        # requires requirements
coordination = true    # requires plans
learning_paths = true
overlays = true
ai = true
```

- Every key defaults to on; a missing file, a missing `[features]` table, or a missing key all mean "on"
- `fubbik serve` reads `./fubbik.toml`, or the file named by `FUBBIK_CONFIG`
- Unknown keys and unknown sections are errors, as is enabling `matrices` without `requirements` or `coordination` without `plans`
- A disabled group's routes are not mounted, and its background jobs do not start (for example, behavior-graph sync with `matrices` off).
  The database schema is always complete; migrations never depend on the switches
- Core (chunks, connections, spaces, tags, search, context, graph, ...) has no switch
- `ai = false` unmounts only the `/api/ai` routes. Enrich, semantic search, check-similar and vocabulary suggest stay on; they already
  degrade without Ollama
- `GET /api/capabilities` (unauthenticated) returns `{ "features": [...] }` with the enabled keys. The web app reads it through
  `apps/web/src/features/nav/use-capabilities.ts`, hides nav and command-palette entries for disabled groups, and shows a turned-off page
  for their routes (`FeatureRouteGuard`, mounted as `InnerWrap` in `apps/web/src/router.tsx`)

