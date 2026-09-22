# Fubbik browser tests

The helpers in `support/ui` adapt Pruva's composition approach to Fubbik's actual shadcn/Base UI components. They are local test infrastructure; the application does not import them and no separate component library is installed.

## Run

```sh
pnpm --filter web exec playwright install chromium
pnpm --filter web run check-types:e2e
pnpm --filter web run test:components
```

The component suite starts an isolated Vite fixture on `127.0.0.1:4178` and imports `src/components/ui` directly. It needs no API server, database, authentication, or local `.env` configuration. Its configuration intentionally refuses to attach to an existing server.

The existing full-stack suites keep their separate command and scratch-database configuration:

```sh
pnpm --filter web run test:e2e
```

Those suites still require the server setup in `playwright.config.ts`. `test:e2e` excludes component specs. The helper migration preserves their existing Node/Rust API assertions, session assertions, and reload/persistence checks; it does not modernize that older dual-server harness.

## Write a test

Import the extended fixture rather than `test` directly from Playwright:

```ts
import { defineForm, expect, test } from "./support/test";

test("updates Chunk settings", async ({ page, ui }) => {
    await page.goto("/your-screen");
    const formUI = ui.within(page.getByRole("form", { name: "Chunk settings" }));
    const settings = defineForm({
        title: formUI.input("Title"),
        type: formUI.select("Chunk type", {
            options: { note: "Note", document: "Document" }
        }),
        pinned: formUI.checkbox("Pinned")
    });

    await settings.fill({ title: "Architecture", type: "note", pinned: true });
    await settings.patch({ pinned: false });
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

## Structure and extension

```text
e2e/
  auth.spec.ts / critical-path.spec.ts  # Existing full-stack scenarios
  support/
    test.ts                            # ui and screen fixtures
    screens/                           # Fubbik workflows and locator bindings
    ui/                                # Shared helpers and typed form composition
  components/
    helpers.spec.ts                     # Real Base UI integration tests
    fixture/                           # Minimal Vite host importing actual components
  type-tests/helpers.ts                # Positive and @ts-expect-error contracts
```

The new E2E TypeScript project enables `exactOptionalPropertyTypes`, `noUncheckedIndexedAccess`, and `noImplicitOverride`. It is included by the web package's normal `check-types` command. Negative type tests are compiled, never executed. Add a regression case when a helper interface changes.

This integration starts with controls and compositions used by Fubbik: inputs, choices, forms, overlays, dialogs, menus, disclosures, and typed tables. It does not copy Pruva's unrelated demo catalog or claim support for components Fubbik does not contain. Add new helpers at this seam when an actual Fubbik workflow needs them, with a real-component scenario and appropriate type contracts.
