# Four required questions — answered with evidence

## Q1: Is `GET /api/plans/{id}/analyze` an object keyed by kind, not an array?

**Confirmed, against real bytes.** `tests/fixtures/node-contract-2c/plans-detail-analyze.json`
(captured live from `GET /api/plans/eedde7c7-e9e7-4004-a79d-7af416cc2537/analyze`):

```json
{"chunk":[{...}],"file":[],"risk":[{...}],"assumption":[{...}],"question":[]}
```

Top-level shape is a plain object with exactly five fixed keys — `chunk`, `file`, `risk`,
`assumption`, `question` — each mapping to an array, always present even when empty (`file` and
`question` are `[]` here, not omitted). Source: `packages/api/src/plans/analyze.ts:14-28,40-48`
(`groupByKind`) initialises all five keys unconditionally before bucketing, so the shape is
stable regardless of what data exists. Confirmed as the one outlier among this domain's
list-shaped GETs — every other plan/search GET in this capture returns a bare array or a
non-kind-keyed object.

---

## Q2: `near:` and `similar-to:` clauses — does `graphMeta` appear, and is `graphMeta.type` the string `"semantic"`?

**Answered from source, not live execution** — `POST /search/query` is a mutating-method
endpoint and the run's authorization is "Issue GETs only", which takes precedence over the
brief's aspiration to hit this route directly (see `_mutating.md`'s rationale note; this is the
one brief instruction this capture explicitly deviates from). The answer is nonetheless
unambiguous from source, because a TS type annotation has zero effect on the runtime value a
`JSON.stringify` emits — only the literal assigned matters, and `as any` doesn't change what
that literal is.

`packages/api/src/search/types.ts:51-53` declares:
```ts
graphMeta?: {
    type: "neighborhood" | "path" | "requirement-reach";
    ...
```
— a three-literal union, exactly as the task brief states.

`packages/api/src/search/service.ts:88-127` (`executeSearch`'s graph-clause loop):
- `near:` clause (line 90-95): `graphMeta = { type: "neighborhood", referenceChunk: clause.value };` — a `near:` query that resolves to at least one neighborhood id **does** produce `graphMeta` in the response (the whole result is only short-circuited to `{chunks:[],total:0,graphMeta}` early-return, still carrying `graphMeta`, if the graph clause resolves to zero ids — `service.ts:130-132`). So yes, results (or an explicit empty set) are returned, and `graphMeta` is present whenever a graph clause is in the query at all — never entirely absent for a `near:` query.
- `similar-to:` clause (line 116-126): `graphMeta = { type: "semantic" as any, referenceChunk: clause.value };` — line 123's exact text, confirming the task brief's premise precisely (`service.ts:123`, not a line-number guess). The `as any` is purely a compile-time escape hatch to let a fourth literal past the type-checker; **the runtime value serialized into the JSON response is the JS string `"semantic"`**, unaffected by the cast. So: yes, `graphMeta.type` is literally `"semantic"` on the wire for `similar-to:` queries — a fourth value the declared TS union does not admit, and any Rust port typing this as a 3-variant enum will panic/reject on deserializing a real `similar-to:` response unless `semantic` is included.

**Caveat:** this is confirmed by code, not by an observed response body (no `POST` was issued to
the live server). Given the code path has no branch that could produce anything other than the
literal string `"semantic"` here (it's a hardcoded object literal, not a computed value), this is
about as certain as source-reading gets — but if a Rust implementer needs an actual captured
`similar-to:` response body byte-for-byte, that capture should be taken against a disposable/seed
database in a follow-up task, not this run.

---

## Q3: `GET /api/search/parse?q=...` — full clause arrays for all twelve exact strings

All twelve captured as raw bytes in `tests/fixtures/node-contract-2c/search-parse-*.json`
(URL-encoded per the brief). Full results, byte-for-byte:

| Query string | Clauses |
|---|---|
| `connections:3` | `[{"field":"connections","operator":"is","value":"3"}]` |
| `connections:3+` | `[{"field":"connections","operator":"gte","value":"3"}]` |
| `updated:30` | `[{"field":"updated","operator":"is","value":"30"}]` |
| `updated:30d` | `[{"field":"updated","operator":"within","value":"30"}]` |
| `Tag:api` | `[{"field":"Tag","operator":"is","value":"api"}]` |
| `affected-by:x hops:3` | `[{"field":"affected-by","operator":"is","value":"x"}]` |
| `near:"Auth Flow" hops:2` | `[{"field":"near","operator":"is","value":"Auth","params":{"hops":"2"}},{"field":"text","operator":"contains","value":"Flow\""}]` |
| `path:"A"->"B"` | `[{"field":"path","operator":"is","value":"A","params":{"from":"A","to":"B"}}]` |
| `NOT tag:deprecated` | `[{"field":"tag","operator":"is","value":"deprecated","negate":true}]` |
| `tag:a,b` | `[{"field":"tag","operator":"any_of","value":"a,b"}]` |
| `"quoted phrase"` | `[{"field":"text","operator":"contains","value":"quoted phrase"}]` |
| `bare words here` | `[{"field":"text","operator":"contains","value":"bare"},{"field":"text","operator":"contains","value":"words"},{"field":"text","operator":"contains","value":"here"}]` |

**Three of these are genuine parser quirks, load-bearing for an exact port — every implementer
would "fix" these unless told not to** (the exact failure mode this whole capture task exists to
prevent). Traced against `packages/api/src/search/parser.ts` (read-only reference; not the file
under concurrent edit — that's the Rust port at `crates/fubbik-api/src/search/parser.rs`):

1. **`Tag:api` keeps the field name's original case — `"Tag"`, not `"tag"`.** The parser
   (`parser.ts:127`, `const field = token.slice(0, colonIdx);`) never lowercases the field name.
   A case-sensitive field means `Tag:api` silently fails to match the `"tag"` case in
   `buildListChunksParams` (`search/service.ts:50-51`), so this clause is effectively a no-op
   filter in practice, not a `tag` filter with different casing — worth flagging loudly for the
   port, since "obviously" lowercasing the field would silently change matched behavior.

2. **`affected-by:x hops:3` drops the `hops:3` token entirely — it never attaches to the
   `affected-by` clause and produces no clause of its own.** `parser.ts:113-122`'s
   `hops:N` special case hardcodes `c.field === "near"` when searching for a clause to attach to
   (`parser.ts:116`); `affected-by` is not `near`, so `lastNear` is `undefined`, the branch's
   `if (lastNear)` guard fails, and the token is discarded with no clause emitted. This is a real
   functional bug, not documentation drift — `search/service.ts:110` reads
   `clause.params?.hops` for `affected-by` clauses too (defaulting to `2` hops when absent), so
   `hops:3` typed after `affected-by:x` in the query bar can **never** actually override the
   default in the current Node implementation. A faithful port must reproduce this silent drop,
   not "fix" it into working.

3. **`near:"Auth Flow" hops:2` doesn't capture the full quoted phrase as the near value.** The
   tokenizer (`parser.ts:12-42`) only treats a `"` as opening a quoted token when it is the
   **first character encountered after skipping whitespace** — it does not special-case a quote
   appearing mid-token immediately after `field:`. For `near:"Auth Flow" hops:2`, the tokenizer
   produces three whitespace-delimited tokens: `near:"Auth`, `Flow"`, `hops:2` (the space inside
   the quoted phrase splits it in two, because by the time the tokenizer reaches that space it's
   already mid unquoted-token-scan). The result: `near`'s value ends up as just `"Auth"` (the
   `Flow"` remainder falls through to the bare-text branch and becomes a stray
   `{field:"text",operator:"contains",value:'Flow"'}` clause — note the **literal trailing quote
   character preserved in the value string**, since the bare-text branch does no quote-stripping
   at all). This directly contradicts the parser's own JSDoc example one line above the function
   (`parser.ts:94`: `near:"Auth Flow" hops:2 → near clause with params.hops` — implying the full
   phrase is captured, which the code does not actually do). The captured bytes are the ground
   truth here, not the docstring or the plan's assumption; a Rust port must replicate the buggy
   split-on-whitespace behavior exactly, including the stray text clause with its embedded `"`,
   for output parity.

`path:"A"->"B"` (no spaces at all in the input) sidesteps the whitespace-splitting bug entirely
— it's a single unquoted token from the tokenizer's point of view, so the `path` branch's own
internal `->`-splitting and quote-stripping (`parser.ts:71-76`, plus the outer per-token
quote-strip at `parser.ts:131`) runs on the whole thing correctly and produces the expected
`{from:"A",to:"B"}`.

---

## Q4: `chunk_staleness.reason` values — does `diverged_duplicate` have a real writer? Does `file_changed` have zero writers?

**Both confirmed exactly as the brief anticipated: `diverged_duplicate` has no writer in any
code path that runs in this app, and `file_changed` has zero writers anywhere.**

Repo-wide grep (including `apps/` and `scripts/`, as instructed) for every reason string:

- **`age`** — written by `packages/db/src/repository/staleness.ts:113-149`
  (`detectAgeStaleChunks`, invoked by `POST /chunks/stale/scan-age`).
- **`requirement_uncovered`** — written by `staleness.ts:187-225` (`detectUncoveredChunks`, same
  route).
- **`requirement_failing`** — written by `staleness.ts:230-265` (`flagRequirementFailing`,
  called from the requirements domain).
- **`upstream_impact`** — written by `packages/api/src/staleness/detect-impact.ts:5-32`
  (`flagImpactRipple`, invoked by `POST /chunks/:id/scan-impact`).
- **`diverged_duplicate`** — the only INSERT anywhere in the repo that writes this reason is
  `packages/db/src/seed.ts:2724-2731` (a legacy, 126KB monolithic seed file dated to the original
  repo scaffold). **That file is dead code**: `pnpm seed` resolves to
  `packages/db/package.json:20` → `bun run src/seed/index.ts`, the modern modular seed pipeline
  under `packages/db/src/seed/`, whose import list (`seed/index.ts:37-56`) never references
  `../seed.ts` and whose `modules/*` files contain zero `chunkStaleness`/`chunk_staleness`
  inserts — `chunks.ts:854-859` only *mentions* `diverged_duplicate` inside a seeded chunk's
  **content text** (documentation about the feature, not a table write). Grepping the whole repo
  (`apps/`, `scripts/`, `packages/`) for `diverged_duplicate` outside that dead file surfaces only:
  the Drizzle schema comment (`schema/staleness.ts:15`), the `WHERE reason = 'diverged_duplicate'`
  read-side clause inside `suppressDuplicatePair` (`repository/staleness.ts:105` — a query that
  can only ever match zero rows in practice, since nothing writes this reason), and frontend
  display-only code (`apps/web/src/features/staleness/staleness-banner.tsx:24,33,75` and its
  built `apps/web/dist/**` bundle) that renders an icon/label *if* such a row ever existed. No
  runtime writer exists. Matches "an earlier analysis could not locate one" exactly — confirmed,
  not merely repeated.
- **`file_changed`** — grep across the entire repo for the literal string turns up **zero writer
  matches of any kind, dead or live**: only the same schema comment
  (`schema/staleness.ts:15`), the same seed-module documentation text
  (`seed/modules/chunks.ts:857`), and the same frontend display-only switch statement. No table
  ever gets a `file_changed` row inserted, confirmed with zero writers as the brief asked to
  verify.

**Bottom line for a Rust port**: `GET /api/chunks/stale` in this live DB (0 undismissed flags
today) can only ever surface `age`, `requirement_uncovered`, `requirement_failing`, or
`upstream_impact` in practice — `diverged_duplicate` and `file_changed` are schema-legal values
with UI support to *display* them, but no code path in this codebase (source or dead) will ever
produce a live `file_changed` row, and only a non-executed dead seed script would ever produce a
`diverged_duplicate` one. An implementer should still accept both reason strings as valid input
(the column has no CHECK constraint tying it to the four "active" reasons — it's a plain `text`
column, `schema/staleness.ts:15`), just not expect to see them in practice.
