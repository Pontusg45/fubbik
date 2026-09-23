# Fubbik browser and end-to-end tests

This directory contains two Playwright suites. **Site E2E tests** exercise the real web app, Rust API, authentication, and a dedicated PostgreSQL database. **Browser component tests** render Fubbik's actual UI components in a small Vite fixture, without the API or database. Both suites share the typed helpers in `support/`; these helpers belong to the test harness and are never imported by the application.

## Contents

- [Choose and run a suite](#choose-and-run-a-suite)
- [Test environment and database safety](#test-environment-and-database-safety)
- [Visual step reporting](#visual-step-reporting)
- [What the suites cover](#what-the-suites-cover)
- [Write a site journey](#write-a-site-journey)
- [Write a component test](#write-a-component-test)
- [Write a test](#write-a-test)
- [Reuse application screens](#reuse-application-screens)
- [Type-safe helper contracts](#type-safe-helper-contracts)
- [Structure and extension](#structure-and-extension)
- [Debug a failure](#debug-a-failure)
- [CI and maintenance](#ci-and-maintenance)

## Choose and run a suite

Run commands from the repository root. Install workspace dependencies first with `pnpm install`; the site suite also needs Bun, the repository's Rust toolchain, Docker, and Playwright Chromium. The component suite needs Playwright Chromium, Firefox, and WebKit. On Linux or CI, `playwright install --with-deps` also installs browser system libraries.

| Goal | Command |
| --- | --- |
| Type-check test helpers and fixtures | `pnpm --filter web run check-types:e2e` |
| Run all component tests | `pnpm --filter web run test:components` |
| Run all site E2E tests | `pnpm --filter web run test:e2e` |
| Run one site spec | `pnpm --filter web run test:e2e graph-matrix-journeys.spec.ts` |
| Run matching site tests | `pnpm --filter web run test:e2e --grep "matrix"` |
| Run one component spec | `pnpm --filter web run test:components helpers.spec.ts` |
| Open the site Playwright report | `pnpm --filter web exec playwright show-report playwright-report/e2e` |
| Open the component Playwright report | `pnpm --filter web exec playwright show-report playwright-report/components` |

Install browsers once on a new machine:

```sh
pnpm --filter web exec playwright install chromium firefox webkit
```

The component runner uses `playwright.components.config.ts`, starts the fixture at `127.0.0.1:4178`, and runs contracts in Chromium, Firefox, and WebKit with two workers. It needs no API server, database, authentication, or local `.env`. The site runner uses `playwright.config.ts`, starts the Rust API on port `3100` and Vite on port `3001`, and runs in Chromium. Both configurations refuse to reuse an existing server, preventing accidental attachment to a developer process.

Arguments after `test:e2e` or `test:components` are forwarded to Playwright. For example, add `--workers=2` to limit parallel site load, `--headed` to watch the browser, or `--grep` to isolate a case. The site suite excludes `components/` and `type-tests/`; the component suite only collects `components/*.spec.ts`.

### Test environment and database safety

The site runner **starts the application servers but does not start PostgreSQL**. Start the dedicated database before running site tests and stop it afterward:

```sh
./scripts/rust-test-db.sh start
pnpm --filter web run test:e2e
./scripts/rust-test-db.sh stop
```

The script manages only the `fubbik-rust-test-db` container, normally bound to `localhost:5434`. Its `url` and `verify` commands print the connection string and check the running profile. The default database URL in `playwright.config.ts` points to this scratch database. Set `E2E_DATABASE_URL` only to a separate disposable test database; the Rust server applies migrations to whichever database that variable names. Do not point it at a development or production database. CI instead provides its own PostgreSQL service through `E2E_DATABASE_URL`.

Every site test should create a unique account with `testAccount()` and `screens.auth.signUp()`, use the `siteTest` fixture for automatic registration, or use an explicit account from its scenario. This isolates user-owned records across tests and workers. Seed other prerequisite records through the authenticated browser request context when doing so makes the page behavior under test clearer. The UI action being verified should still happen in the browser. Tests that prove persistence should reload or read the real API after the action.

The site harness compiles Rust from checked-in SQLx metadata (`SQLX_OFFLINE=true`) and lets Rust migrate the scratch database on startup. A cold build can take several minutes; `playwright.config.ts` allows five minutes for the API to start. The auth secret and development-UX settings in that configuration are dedicated to this harness.

## Visual step reporting

Add `--step-screenshots` to either test command to attach one screenshot to each named helper step. The target control is outlined in orange at capture time; workflow and API steps without a target show the result page. The outline is removed before the action continues. This is off by default to keep routine reports small. `--no-step-screenshots` explicitly disables it, and the last flag wins if both are supplied.

```sh
pnpm --filter web run test:e2e --step-screenshots auth.spec.ts
pnpm --filter web run test:components --step-screenshots helpers.spec.ts
pnpm --filter web exec playwright show-report playwright-report/e2e
```

To export a report file, add `--step-report=html`, `--step-report=md`, or `--step-report=both`:

```sh
pnpm --filter web run test:e2e --step-screenshots --step-report=both auth.spec.ts
pnpm --filter web run test:components --step-screenshots --step-report=md helpers.spec.ts
```

The exports are `test-results/e2e/step-report.html` and/or `step-report.md` for the full-stack suite, and the same names under `test-results/components` for component tests. The HTML file embeds screenshots and can be shared by itself. The Markdown file links to PNGs in its adjacent `step-report-assets` directory; keep that directory with the Markdown file when sharing it. Exports include test status, duration, named step hierarchy, errors, and available images. They can also be generated without `--step-screenshots` when a text-only step summary is wanted. The normal Playwright HTML report remains available.

The screenshots appear as attachments inside their Playwright steps in the HTML report. Playwright's usual failure screenshot and trace remain enabled independently of this flag. For a test-specific Given/When/Then step, use `reportStep` with a page or the action's locator:

```ts
import { reportStep } from "./support/test";

await reportStep("When the user saves the form", ui.button("Save").root, async () => {
    await ui.button("Save").click();
});
```

For locator targets, the image is captured immediately before the action so a button that disappears after a click is still highlighted. For page targets, it is captured after the action. Every form field and existing named workflow step uses this helper; bare Playwright calls outside a named step can be wrapped explicitly when visual evidence is useful.

### What a screenshot represents

`reportStep(title, target, action)` is a Playwright step whether screenshots are enabled or not. A **locator** target captures just before the action and temporarily outlines the visible target in orange. A **page** target captures after the action. API/setup steps may therefore show the resulting page without a highlighted control. The capture code restores inline styles and records capture errors as attachments so a screenshot failure does not hide the action result. Raw Playwright actions do not acquire step screenshots automatically; wrap important transitions in `reportStep` or the `given`/`when`/`then` helpers.

The runner accepts `--no-step-report` to cancel an earlier report flag. Invalid formats fail fast. Screenshot attachments and generated reports are separate from Playwright's failure screenshot and retained-on-failure trace, which remain enabled by both configurations.

## What the suites cover

Site specs under `e2e/*.spec.ts` use the actual application. The groupings below are a navigation aid rather than a fixed test count; run `pnpm --filter web run test:e2e --list` for the current cases.

| Area | Specs | Representative behavior |
| --- | --- | --- |
| Authentication and account boundaries | `auth`, `auth-boundaries`, `account-isolation` | Sign-up, session behavior, duplicate accounts, private data across users |
| Chunks and browsing | `critical-path`, `chunk-workflows`, `chunk-editor-more`, `chunk-browsing-more`, `chunk-filters`, `chunk-archive` | Validation, edits, reload persistence, filters, archive/restore, failed write and retry |
| Spaces, features, import | `space-workflows-more`, `feature-workflows`, `features-more`, `import-workflows`, `import-wizard-more` | Scoped browsing, lifecycle actions, folder preview, import failures and retry |
| Plans and requirements | `plan-workflows`, `plan-tasks-more`, `requirements-more` | Tasks, dependencies, ordering, criteria, Given/When/Then requirements and links |
| Search and vocabulary | `search-activity-more`, `tags-templates-more`, `settings-vocabulary` | URL-backed queries, activity filters, tags, templates and settings |
| Graphs and matrices | `graph-matrix-journeys` | Connected graph nodes, matrix rules, dimensions, cells and coverage |
| Review and health | `review-health-journeys` | Proposal decisions, bulk review, orphan and thin-content health |
| Compare, compose and context | `compare-compose-context-journeys` | Selection, diff URLs, composition export and file context |
| Learning and coverage | `learning-coverage-journeys` | Path order and requirement coverage links |
| Workspaces | `workspace-journeys` | Membership, deletion behavior and cross-account privacy |

The import fixtures are in `fixtures/import-docs/`. They include Markdown inputs and a non-Markdown file that the folder picker should ignore. Component specs under `components/` cover controls and forms (`helpers`, `fields`), overlays and keyboard interaction (`overlays`, `keyboard`), invalid bindings and disabled actions (`failures`), request observation (`network`), and screenshot step behavior (`reporting`). The fixture host in `components/fixture/` imports the real UI components; it is not a mock of a deployed page.

## Write a site journey

For journeys that start signed in, import `siteTest` from `support/site-test`. Its automatic `account` fixture registers a fresh user before each test; its `site` fixture provides the authenticated `{ request, origin }` API context. Use the base `test` from `support/test` for login, registration, or anonymous scenarios where automatic sign-up would change the behavior under test. The `support/site-scenarios.ts` helpers provide API setup (`apiJson`, `seedChunk`, `seedSpace`, `seedConnection`, `seedWorkspaceWithSpace`, `seedRequirement`, `seedCoveredChunk`), page opening (`openSite`), and readable step names (`given`, `when`, `then`). A minimal pattern is:

```ts
import { given, openSite, seedChunk, then, when } from "./support/site-scenarios";
import { siteTest as test } from "./support/site-test";
import { expect } from "./support/test";

test("a saved chunk is visible in the browser", async ({ page, site }) => {
    // Given a signed-in account and a saved chunk.
    const chunk = await given(page, "a saved chunk", () => seedChunk(site, "Architecture note"));

    // When its detail page is opened.
    await when(page, "the chunk page opens", () => openSite(page, `/chunks/${chunk.id}`));

    // Then the saved title is visible after reload.
    await then(page, "the title persists", async () => {
        await page.reload();
        await expect(page.getByRole("heading", { name: "Architecture note" })).toBeVisible();
    });
});
```

Keep the Given/When/Then comments in test bodies. Name the major `given`/`when`/`then` steps so reports are readable; the comment alone does not create a report step. Prefer roles, labels and accessible names for locators. Scope repeated labels to a dialog, form or table. When UI work triggers an API request, `network.perform({ method, path, status }, action)` installs the response listener before the action and asserts the exact method, origin, pathname and status. Avoid arbitrary sleeps; wait for a visible state, response, URL change, or persisted API result. Use UI interactions for the behavior the test claims to verify, and API seeding only for setup that is incidental to that behavior.

Use a test-local account and unique names for any records that can collide. Keep cross-user isolation in a single test with separate browser contexts or accounts rather than depending on test order. A success workflow should assert both what the user sees and, when relevant, that the result survives a reload. A validation or failure-path test should use the lower-level action (`create()`, `save()`, `submit()`) because success helpers deliberately require a successful response and navigation.

## Write a component test

Component specs import the same extended fixture with `../support/test`, navigate to `/`, and bind helpers to the rendered fixture. They verify interactions and accessibility contracts of the real Fubbik components without asserting application data or calling the Rust API. Add or adjust a fixture component in `components/fixture/` only when an actual component contract needs coverage. Keep any `@ts-expect-error` helper contracts in `type-tests/helpers.ts`; they are compiled by `check-types:e2e`, not run in a browser.

```ts
import { expect, test } from "../support/test";

test("a title field accepts input", async ({ page, ui }) => {
    // Given the component fixture and its settings form.
    await page.goto("/");
    const title = ui.within(page.getByRole("form", { name: "Chunk settings" })).input("Title");

    // When the title changes.
    await title.fill("Architecture");

    // Then the control exposes the new value.
    await title.expectValue("Architecture");
    await expect(title.root).toBeVisible();
});
```

## Write a test

Import the extended fixture rather than `test` directly from Playwright:

```ts
import { defineForm, expect, test } from "./support/test";

test("updates Chunk settings", async ({ page, ui }) => {
    // Given a scoped Chunk settings form.
    await page.goto("/your-screen");
    const formUI = ui.within(page.getByRole("form", { name: "Chunk settings" }));
    const settings = defineForm({
        title: formUI.input("Title"),
        type: formUI.select("Chunk type", {
            options: { note: "Note", document: "Document" }
        }),
        pinned: formUI.checkbox("Pinned")
    });

    // When the form is filled and its pinned state patched.
    await settings.fill({ title: "Architecture", type: "note", pinned: true });
    await settings.patch({ pinned: false });
    // Then the unpatched type remains Note.
    await settings.fields.type.expectValue("note");
});
```

The option map separates application keys from visible labels. TypeScript infers `"note" | "document"`, boolean checkbox/switch values, and string inputs. Keep literal bindings at definition time (or use `as const` when extracting a reusable map); a broad `Record<string, string>` necessarily loses the finite option union.

`fill()` requires all fields. `patch()` accepts a subset. Both reject extra fields, including extra keys in a variable. Runtime validation checks every supplied value before the first browser interaction, protecting JavaScript callers too. Fields are updated sequentially because overlays and focus are shared page state. Each field produces a named Playwright step without putting field values into step titles.

## Reuse application screens

The `screens` fixture provides per-page, isolated bindings:

```ts
await screens.auth.signIn({ email: user.email, password: user.password });
await screens.chunks.openNew();
await screens.chunks.form.fill({ title: "Decision", content: "Use PostgreSQL." });
await screens.chunks.addTag("architecture");
await screens.chunks.setDecisionContext({
    alternatives: ["SQLite", "PostgreSQL"],
    consequences: "Requires a server."
});

// Keep network and domain assertions in the test.
const created = page.waitForResponse(response =>
    response.url().endsWith("/api/chunks") && response.request().method() === "POST"
);
await screens.chunks.create();
expect((await created).status()).toBe(201);
```

Auth screen bindings centralize the existing `networkidle` readiness wait. This preserves the previous suite's behavior; it is not a guarantee of hydration. Replace it with an application-owned readiness signal if the auth pages gain long-lived connections or delayed hydration.

## Type-safe helper contracts

- `Surface` offers visibility/text assertions. Only controls expose disabled assertions.
- Checkbox supports mixed state. Switch supports only boolean state. Radio offers selection, not an impossible “uncheck this radio” operation.
- `select()` returns a single-select helper. `multiSelect()` returns a different helper with array values and `setSelected()`. The option map for a multiselect should enumerate all managed options; `set()` clears omitted mapped options.
- Select assertions bind Fubbik's `select-value` slot; custom markup can provide a `value` locator.
- Dialog, Sheet, Menu, and Disclosure reuse buttons/controls. Overlay opening returns a resolved `Surface`; callers never read a mutable, unresolved content locator.
- Portals follow `aria-controls` in the trigger's owning frame. Explicit `content` bindings handle custom layouts. Ambiguous matches fail rather than choosing the first element.
- `within()` accepts a locator, page, or frame. Portaled dialogs outside a local form should be retrieved from the outer `ui` or bound explicitly.
- Dynamic values and the actual DOM still need runtime checks. A TypeScript option map cannot prove that the application rendered those options.

## Typed tables

```ts
const chunks = ui.table("Chunks", {
    columns: {
        title: { label: "Title", sortable: true },
        type: { label: "Type" }
    }
});
await chunks.sortBy("title", "ascending");
await chunks.row({ column: "title", value: "Architecture" }).expectCellText("type", "Note");
```

Unknown columns and sorting an undeclared sortable column fail compilation. Cells resolve lazily inside `expectCellText()`, avoiding nested awaits. Rows match a specific column and must be unique. The contract is a semantic table with one header row and rendered tbody rows; grouped headers, virtualized data, and fetching across server pages need explicit application adapters.

## Menus, popovers, sheets, and disclosures

```ts
const actions = ui.dropdownMenu("Chunk actions");
const menu = await actions.open();
await menu.checkbox("Show archived").set(true);
await menu.radio("Grid view").choose();
await menu.choose("Export chunk");

const summary = ui.popover("Edit summary");
const content = await summary.open();
await ui.within(content.root).input("Title").fill("Summary");
await summary.close(); // Escape

const inspector = ui.sheet("Chunk inspector");
await inspector.open(ui.button("Open inspector"));
await inspector.dismiss(); // Escape; close() clicks the named Close button

await ui.disclosure("Decision context").expand();
```

Menu checkbox and radio items use their ARIA roles and retain the control helpers' state assertions. If an item closes the menu, call `actions.open()` again before interacting with another item. The fixture's preference items use `closeOnClick={false}` to allow consecutive selections.

Popover `open()` returns a resolved surface and can be repeated while open; after `close()`, the same helper can reopen it. Use `content` for an explicit popup locator, including one inside a frame. Sheets reuse the dialog contract. Disclosures work with both Collapsible and Accordion triggers; repeated `expand()` or `collapse()` calls preserve the requested state.

The component suite covers menu state and reopening, popover scope and frame bindings, sheet close/dismiss with focus restoration, and idempotent disclosure actions against the actual Fubbik components.

## Structure and extension

```text
e2e/
  *.spec.ts                         # Real-site journeys through Rust and Vite
  fixtures/import-docs/             # Files used by folder-import scenarios
  support/
    test.ts                         # Playwright fixtures: ui, screens, network, apiOrigin
    data.ts                         # Unique test accounts
    site-scenarios.ts               # API setup and Given/When/Then report steps
    site-test.ts                    # Automatic signed-in account and site API fixture
    network.ts                      # Request observation and scoped failure injection
    reporting.ts                    # Named steps and optional highlighted screenshots
    screens/                        # Auth, chunk, feature and space workflows
    ui/                             # Typed control, form, overlay and table helpers
  components/
    *.spec.ts                       # Real-component behavior in an isolated browser host
    fixture/                       # Vite host importing Fubbik UI components
  type-tests/helpers.ts             # Positive and @ts-expect-error contracts
```

The E2E TypeScript project enables `exactOptionalPropertyTypes`, `noUncheckedIndexedAccess`, and `noImplicitOverride`. It is included by the web package's normal `check-types` command. Negative type tests are compiled, never executed. Add a regression case when a helper interface changes. `apps/web/scripts/run-e2e.mjs` parses the step-reporting flags and forwards other arguments to Playwright; `step-file-reporter.mjs` writes the standalone HTML/Markdown exports. Its focused reporter tests run with `pnpm --filter web run test:reporting`.

Put a reusable **application workflow** in `support/screens/`, a reusable **domain setup or journey step** in `support/site-scenarios.ts`, and a reusable **control behavior** in `support/ui/`. Keep assertions specific to one feature in its spec. When the same sequence appears across tests, add a helper at the narrowest shared seam and make its success conditions explicit. Extend controls and compositions that Fubbik actually uses: inputs, choices, forms, overlays, dialogs, menus, disclosures and typed tables. A helper interface change should have a real-component scenario and type contract.

### Native selects and tag inputs

Use `nativeSelect` for HTML `<select>` elements. Its map binds typed keys to DOM option **values**; `select` continues to target Base UI dropdowns.

```ts
const type = ui.nativeSelect("Type", { note: "note", document: "document" });
await defineForm({ type }).fill({ type: "document" });
await type.expectValue("document");

const container = page.getByRole("form", { name: "Chunk settings" });
const tags = ui.within(container).tagInput("Tags", {
    normalize: value => value.trim().toLowerCase(),
    chip: value => container.getByText(`${value} ×`, { exact: true })
});
await tags.add(" Architecture ");
await tags.expectTag("architecture");
await tags.remove("architecture");
```

Tag inputs require an explicit chip locator and optionally a separate `remove` locator callback for chips with removal buttons. `add` submits with Enter and waits for one chip and an empty input. Blank tags fail before interaction. Tags expose individual actions, not form replacement semantics. The chunk screen binds `tags` and `type` by label on both create and edit pages; existing `addTag` calls remain compatible.

## Compose successful workflows

Use screen workflows for repeated success paths. They check the expected API status and wait for navigation; domain assertions stay explicit in the test.

```ts
import { expect, test, testAccount } from "./support/test";

test("persists a chunk edit", async ({ screens }) => {
    // Given a newly registered user and a persisted chunk.
    await screens.auth.signUp(testAccount());
    const original = { title: "Decision", content: "Use PostgreSQL." };
    const chunk = await screens.chunks.createChunk(original);
    await screens.chunks.expectDetails(original);

    // When its content is edited and saved.
    await screens.chunks.openEdit();
    const updated = { ...original, content: "Use PostgreSQL with AGE." };
    await screens.chunks.form.fill(updated);
    await screens.chunks.saveAndOpen();

    // Then its identity and content survive a reload.
    expect(screens.chunks.current().id).toBe(chunk.id);
    await screens.chunks.reloadAndExpect(updated);
});
```

- `createChunk(values)` opens the new editor, fills it, submits, and returns `{ id, url, path }` on the detail page.
- `createAndOpen()` submits an already configured editor, including any tags or decision context.
- `saveAndOpen()` saves the current editor and waits for that same chunk's detail page.
- `expectDetails(values)` checks the title and exact plain-text content; `reloadAndExpect(values)` also reloads first. For rendered Markdown, use assertions appropriate to its rendered elements.
- `expectDecisionContext(values)` opens the context drawer and checks its alternatives and consequences within that drawer; it returns the drawer for explicit dismissal.
- `feature.createFeature(name)` opens, fills and submits the feature dialog.
- `spaces.create(name)` creates a space and returns its ID for workflows such as document import.
- `testAccount(overrides)` generates unique account credentials; `auth.session()` reads the browser's authenticated session.

Keep `create()`, `save()`, and `feature.submit()` for tests that expect validation or failure. These actions do not assume successful responses or navigation.

## Observe API actions and scope failures

The `network` fixture matches the configured API **origin**, exact pathname and HTTP method. Query strings are ignored. `apiOrigin` defaults to the isolated Rust server (`http://localhost:3100`); override it with `test.use({ apiOrigin })` for a different harness.

```ts
const endpoint = { method: "PATCH", path: screens.chunks.current().path } as const;
const writes = network.record(endpoint);
await network.withFailure(endpoint, async () => {
    await network.perform({ ...endpoint, status: 500 }, () => screens.chunks.save());
    expect(writes.requests).toHaveLength(1);
});
await screens.chunks.saveAndOpen();
```

`perform` installs its listener before running the action, checks the supplied status, returns the response, and removes its listener on success, timeout or action failure. Its timeout defaults to 15 seconds and can be overridden per call. `record` exposes requests for assertions and has a `stop()` method; fixture teardown stops remaining recorders. `withFailure` returns HTTP 500 only for the specified endpoint and method, preserves other route handlers, and removes its own handler in `finally`.

## Debug a failure

Start with the failed case in the terminal output or `apps/web/playwright-report/e2e/` (use `components/` for component tests). Playwright also keeps each run's output in `apps/web/test-results/e2e/` or `apps/web/test-results/components/`. Failure screenshots and traces are captured even when `--step-screenshots` is off. An `error-context.md` file, when present in a failed test's result directory, contains the page snapshot around the failure.

1. Re-run the exact spec or title with `--grep`, using the same suite command. If the case only fails in a larger run, keep the original worker count and related specs while investigating shared state or timing.
2. Check the last named step, the expected URL or response, and the page snapshot. For navigation failures, verify that the expected route actually rendered after the URL changed.
3. Use `--headed` for a visible browser or `--step-screenshots --step-report=html` for a sequence of annotated actions. The latter can produce a large file on a full run, so start with the failing spec.
4. Replace timing guesses with an observable condition: a locator state, a response registered before the click, a URL, or a persisted API value. A screenshot documents state; it does not prove a request completed.
5. Re-run the targeted test after the fix, then the relevant suite. Keep failure injection scoped with `network.withFailure()` so retries and later assertions reach the real API.

If the site suite cannot start, confirm the scratch database with `./scripts/rust-test-db.sh verify`, check that ports `3100` and `3001` are free, and inspect the Rust/Vite startup output. If the component suite cannot start, check port `4178` and the installed browser binaries. Both Playwright configurations deliberately fail rather than reuse an already-running server. Use the test database script's `stop` command for cleanup after an interrupted local run; do not remove unrelated containers.

## CI and maintenance

CI runs component tests in their own job after installing all three browsers. The site E2E job uses a dedicated PostgreSQL service, sets `E2E_DATABASE_URL`, installs Chromium, and runs `pnpm run test:e2e` from the repository root. Both jobs upload their Playwright HTML reports. Locally, `pnpm --filter web run check-types` includes E2E helper and fixture type checks; `pnpm --filter web run lint` includes the E2E TypeScript files.

Before merging a browser-test change, run the smallest relevant spec, then the affected suite, plus type-checking. Run `pnpm --filter web run test:reporting` when changing the report exporter. For a new helper contract, add a component behavior test and update `type-tests/helpers.ts` when type constraints matter. For a new site workflow, include Given/When/Then comments, readable named steps, a unique account, a UI action, and a result assertion that would fail if the behavior regressed.
