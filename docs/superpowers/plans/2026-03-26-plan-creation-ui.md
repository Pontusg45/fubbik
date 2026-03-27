# Plan Creation UI Improvements

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the plan creation form richer with templates, markdown paste, bulk entry, requirement linking, and keyboard shortcuts.

**Architecture:** All changes in `apps/web/src/routes/plans.new.tsx` and supporting components. The form currently has title + description + simple step list. We add: template selector (fetches from API), markdown paste mode (uses existing parser), bulk paste mode, requirement linking per step, codebase selector, autosave, step reorder, and keyboard improvements.

**Tech Stack:** React, TanStack Query, Eden treaty, shadcn-ui (base-ui), existing `useAutosave` hook, existing `parsePlanMarkdown`

**Codebase notes:**
- Plan templates API: `GET /plans/templates` returns `Record<string, { title, description, steps }>`
- Markdown parser: `packages/api/src/plans/parse-plan-markdown.ts` → `parsePlanMarkdown(md)` returns `{ title, description, steps }`
- `useAutosave` hook: `apps/web/src/features/chunks/use-autosave.ts`
- `useActiveCodebase` hook: `apps/web/src/features/codebases/use-active-codebase.ts`
- The form uses raw `<input>` elements — should use `<Input />` from `@/components/ui/input`
- `planStep.requirementId` FK exists (added earlier in this session)

---

## File Structure

### Files to modify:
- `apps/web/src/routes/plans.new.tsx` — Main form page (all features)

---

## Task 1: Input Components + Codebase Selector + Autosave

**Files:**
- Modify: `apps/web/src/routes/plans.new.tsx`

- [ ] **Step 1: Read the file**

Read `apps/web/src/routes/plans.new.tsx` fully to understand the current form structure.

- [ ] **Step 2: Replace raw inputs with `<Input />` and `<Textarea />`**

Import `Input` from `@/components/ui/input`. Replace all raw `<input>` elements with `<Input />`. Replace the raw `<textarea>` with a proper textarea (check if `@/components/ui/textarea` exists; if not, keep `<textarea>` but use `Input`'s styling pattern).

- [ ] **Step 3: Add codebase selector**

Import `useActiveCodebase` from `@/features/codebases/use-active-codebase`. Pass the active `codebaseId` to the create mutation:

**NOTE:** `useActiveCodebase().codebaseId` reads from the URL `?codebase=` param. On `/plans/new` this will be `null` unless the user navigated with the param (e.g., from the requirements page which carries it). This is expected behavior — the plan will be global unless a codebase is active.

```tsx
const { codebaseId } = useActiveCodebase();

// In mutation:
...(codebaseId ? { codebaseId } : {}),
```

- [ ] **Step 4: Add autosave**

Import `useAutosave` and `loadDraft` from `@/features/chunks/use-autosave`. Save form state to `"plan-draft-new"`:

```tsx
const formState = useMemo(() => ({ title, description, steps }), [title, description, steps]);
const { clearDraft } = useAutosave("plan-draft-new", formState);

// On mount, restore draft:
useEffect(() => {
    const draft = loadDraft<typeof formState>("plan-draft-new");
    if (draft?.title || draft?.steps?.some(s => s.description)) {
        setTitle(draft.title ?? "");
        setDescription(draft.description ?? "");
        setSteps(draft.steps ?? [{ description: "" }]);
        toast.info("Restored unsaved plan draft");
    }
}, []);

// On successful create:
clearDraft();
```

Import `DraftIndicator` from `@/features/chunks/draft-indicator` and show near the submit button.

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(web): standardize plan form inputs, add codebase selector and autosave"
```

---

## Task 2: Template Selector

**Files:**
- Modify: `apps/web/src/routes/plans.new.tsx`

- [ ] **Step 1: Fetch templates**

**CRITICAL:** The templates API returns `{ templates: Array<{ key, title, description, stepCount }> }` — an ARRAY wrapped in an object, NOT a Record. And it does NOT include step descriptions (only `stepCount`).

To pre-fill steps from a template, use the `POST /plans` endpoint with the `template` field and let the server expand steps. The template selector should create the plan server-side, then redirect to the detail page where the user can edit.

Alternatively, extend the API to return step descriptions in templates (simpler for the UI). Read `packages/api/src/plans/service.ts` `listPlanTemplates` to understand the current shape and extend it to include `steps: string[]`.

```tsx
const templatesQuery = useQuery({
    queryKey: ["plan-templates"],
    queryFn: async () => {
        const result = unwrapEden(await api.api.plans.templates.get());
        return (result as any)?.templates ?? [];
    },
    staleTime: 60_000,
});
```

- [ ] **Step 2: Extend templates API to return step strings**

In `packages/api/src/plans/service.ts`, modify `listPlanTemplates` to include `steps: string[]` (the actual step descriptions) alongside `stepCount`. Then the frontend can pre-fill.

- [ ] **Step 3: Add template selector UI**

Above the title field, add template buttons:

```tsx
<div className="mb-4">
    <label className="mb-1.5 block text-sm font-medium">Start from template (optional)</label>
    <div className="flex flex-wrap gap-2">
        {(templatesQuery.data ?? []).map((tmpl: any) => (
            <Button
                key={tmpl.key}
                type="button"
                variant="outline"
                size="sm"
                onClick={() => {
                    setTitle(tmpl.title);
                    setDescription(tmpl.description);
                    if (tmpl.steps?.length) {
                        setSteps(tmpl.steps.map((s: string) => ({ description: s })));
                    }
                }}
            >
                {tmpl.title}
            </Button>
        ))}
    </div>
</div>
```

Clicking a template pre-fills title, description, and steps. The user can then edit.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): add template selector to plan creation form"
```

---

## Task 3: Markdown Paste Mode

**Files:**
- Modify: `apps/web/src/routes/plans.new.tsx`

- [ ] **Step 1: Add mode toggle**

Add state for creation mode:

```tsx
const [mode, setMode] = useState<"builder" | "markdown">("builder");
```

Add toggle buttons above the form:

```tsx
<div className="flex gap-1 mb-4">
    <Button variant={mode === "builder" ? "default" : "outline"} size="sm" onClick={() => setMode("builder")}>
        Step Builder
    </Button>
    <Button variant={mode === "markdown" ? "default" : "outline"} size="sm" onClick={() => setMode("markdown")}>
        Paste Markdown
    </Button>
</div>
```

- [ ] **Step 2: Add markdown textarea**

When mode is "markdown", show a large textarea instead of the step builder:

```tsx
const [markdownInput, setMarkdownInput] = useState("");

{mode === "markdown" ? (
    <div>
        <label className="mb-1.5 block text-sm font-medium">Paste plan markdown</label>
        <textarea
            value={markdownInput}
            onChange={(e) => setMarkdownInput(e.target.value)}
            placeholder="Paste a plan markdown file here..."
            rows={15}
            className="w-full rounded-md border bg-background px-3 py-2 text-sm font-mono"
        />
        <Button
            type="button"
            variant="outline"
            size="sm"
            className="mt-2"
            onClick={() => {
                // Parse and switch to builder mode
                // NOTE: parsePlanMarkdown at packages/api/src/plans/parse-plan-markdown.ts
                // is pure TS with no server deps — it CAN be imported directly.
                // But to avoid cross-package imports, duplicate the simple parsing client-side:
                const lines = markdownInput.split("\n");
                let parsedTitle = title;
                let parsedDesc = description;
                const parsedSteps: StepRow[] = [];
                let currentTask = "";

                for (const line of lines) {
                    if (line.startsWith("# ") && !parsedTitle) parsedTitle = line.replace(/^#\s+/, "").trim();
                    if (line.startsWith("**Goal:**")) parsedDesc = line.replace("**Goal:**", "").trim();
                    if (line.match(/^##\s+Task\s+\d+/)) currentTask = line.replace(/^##\s+/, "").trim();
                    const stepMatch = line.match(/^-\s+\[[ x]\]\s+\*\*(?:Step\s+\d+:\s+)?(.+?)\*\*/);
                    if (stepMatch) {
                        const desc = currentTask ? `[${currentTask}] ${stepMatch[1]!.trim()}` : stepMatch[1]!.trim();
                        parsedSteps.push({ description: desc });
                    }
                }

                if (parsedTitle) setTitle(parsedTitle);
                if (parsedDesc) setDescription(parsedDesc);
                if (parsedSteps.length > 0) setSteps(parsedSteps);
                setMode("builder");
                toast.success(`Parsed ${parsedSteps.length} steps from markdown`);
            }}
        >
            Parse & Switch to Builder
        </Button>
    </div>
) : (
    // existing step builder
)}
```

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): add markdown paste mode to plan creation"
```

---

## Task 4: Bulk Step Entry + Step Count

**Files:**
- Modify: `apps/web/src/routes/plans.new.tsx`

- [ ] **Step 1: Add bulk paste button**

Add a "Paste steps" button below the step list that shows a textarea for multi-line entry:

```tsx
const [showBulkEntry, setShowBulkEntry] = useState(false);
const [bulkText, setBulkText] = useState("");

{showBulkEntry ? (
    <div className="mt-2 space-y-2">
        <textarea
            value={bulkText}
            onChange={(e) => setBulkText(e.target.value)}
            placeholder="Paste one step per line..."
            rows={6}
            className="w-full rounded-md border bg-background px-3 py-2 text-sm"
        />
        <div className="flex gap-2">
            <Button type="button" size="sm" onClick={() => {
                const newSteps = bulkText.split("\n").filter(l => l.trim()).map(l => ({ description: l.trim() }));
                setSteps(prev => [...prev.filter(s => s.description.trim()), ...newSteps]);
                setBulkText("");
                setShowBulkEntry(false);
                toast.success(`Added ${newSteps.length} steps`);
            }}>
                Add {bulkText.split("\n").filter(l => l.trim()).length} steps
            </Button>
            <Button type="button" variant="outline" size="sm" onClick={() => setShowBulkEntry(false)}>
                Cancel
            </Button>
        </div>
    </div>
) : (
    <Button type="button" variant="ghost" size="sm" onClick={() => setShowBulkEntry(true)}>
        Paste multiple steps
    </Button>
)}
```

- [ ] **Step 2: Add step count indicator**

Show a count next to the "Steps" label:

```tsx
<label className="mb-2 flex items-center gap-2 text-sm font-medium">
    Steps
    <span className="text-muted-foreground text-xs">
        ({steps.filter(s => s.description.trim()).length} valid)
    </span>
</label>
```

- [ ] **Step 3: Add empty step visual hint**

On each step input, show a yellow left border when the step is empty and other steps are filled:

```tsx
className={`... ${!step.description.trim() && steps.some(s => s.description.trim()) ? "border-l-2 border-l-yellow-400" : ""}`}
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(web): add bulk step entry, step count, and empty step indicator"
```

---

## Task 5: Requirement Linking on Steps

**Files:**
- Modify: `apps/web/src/routes/plans.new.tsx`

- [ ] **Step 1: Extend StepRow with requirementId**

```tsx
interface StepRow {
    description: string;
    requirementId?: string;
}
```

- [ ] **Step 2: Add requirement search per step**

Fetch requirements:
```tsx
const reqsQuery = useQuery({
    queryKey: ["requirements-for-linking"],
    queryFn: async () => unwrapEden(await api.api.requirements.get({ query: {} })),
    staleTime: 30_000,
});
```

On each step row, add a small "Link" button that shows a dropdown of requirements:

```tsx
<Button type="button" variant="ghost" size="sm" className="size-8 p-0" title="Link requirement"
    onClick={() => setLinkingStepIndex(i)}>
    <LinkIcon className="size-3.5" />
</Button>

{linkingStepIndex === i && (
    <div className="absolute z-10 mt-1 w-64 rounded-md border bg-background p-2 shadow-lg">
        {(reqsQuery.data?.requirements ?? []).slice(0, 10).map((req: any) => (
            <button key={req.id} onClick={() => {
                updateStep(i, step.description, req.id);
                setLinkingStepIndex(null);
            }} className="block w-full text-left px-2 py-1 text-sm hover:bg-muted rounded">
                {req.title}
            </button>
        ))}
    </div>
)}
```

Show a small badge when a step has a linked requirement:
```tsx
{step.requirementId && (
    <Badge variant="outline" size="sm" className="text-[10px] shrink-0">req</Badge>
)}
```

Pass `requirementId` through to the create mutation.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): add requirement linking on plan creation steps"
```

---

## Task 6: Step Reorder + Keyboard Shortcuts

**Files:**
- Modify: `apps/web/src/routes/plans.new.tsx`

- [ ] **Step 1: Add up/down arrows for reorder**

On each step row, add small up/down buttons (same pattern as the plan detail page's `PlanStepItem`):

```tsx
<Button type="button" variant="ghost" size="sm" className="size-6 p-0"
    disabled={i === 0}
    onClick={() => {
        const next = [...steps];
        [next[i - 1], next[i]] = [next[i]!, next[i - 1]!];
        setSteps(next);
    }}>
    <ChevronUp className="size-3" />
</Button>
<Button type="button" variant="ghost" size="sm" className="size-6 p-0"
    disabled={i === steps.length - 1}
    onClick={() => {
        const next = [...steps];
        [next[i], next[i + 1]] = [next[i + 1]!, next[i]!];
        setSteps(next);
    }}>
    <ChevronDown className="size-3" />
</Button>
```

- [ ] **Step 2: Add keyboard shortcuts**

Enhance the `onKeyDown` handler on step inputs:

```tsx
onKeyDown={(e) => {
    if (e.key === "Enter") {
        e.preventDefault();
        addStepAfter(i);
        // Focus next step after render
        setTimeout(() => {
            const inputs = document.querySelectorAll<HTMLInputElement>("[data-step-input]");
            inputs[i + 1]?.focus();
        }, 50);
    }
    if (e.key === "Backspace" && !step.description && steps.length > 1) {
        e.preventDefault();
        removeStep(i);
        setTimeout(() => {
            const inputs = document.querySelectorAll<HTMLInputElement>("[data-step-input]");
            inputs[Math.max(0, i - 1)]?.focus();
        }, 50);
    }
    if (e.key === "ArrowUp" && e.altKey) {
        e.preventDefault();
        if (i > 0) {
            const next = [...steps];
            [next[i - 1], next[i]] = [next[i]!, next[i - 1]!];
            setSteps(next);
        }
    }
    if (e.key === "ArrowDown" && e.altKey) {
        e.preventDefault();
        if (i < steps.length - 1) {
            const next = [...steps];
            [next[i], next[i + 1]] = [next[i + 1]!, next[i]!];
            setSteps(next);
        }
    }
}}
data-step-input
```

Add a small hint below the steps:
```tsx
<p className="text-muted-foreground text-xs mt-1">
    Enter: add step · Backspace on empty: remove · Alt+↑/↓: reorder
</p>
```

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): add step reorder arrows and keyboard shortcuts to plan form"
```
