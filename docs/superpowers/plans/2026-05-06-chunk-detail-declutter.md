# Chunk Detail Page Declutter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Simplify the chunk detail page from a cluttered 3-column layout with inline sections to a clean 2-column layout (content + metadata), relocating decision context, proposals, and feature overlays into the existing More Context drawer.

**Architecture:** Remove the left sibling navigator entirely (delete component + file). Move the decision context callout from `ChunkDetailContent` to the Context tab of the drawer. Move proposals from inline to the Links tab. Move feature overlays from inline to the Context tab. Thread new props through the drawer component to its tab children.

**Tech Stack:** React, TanStack Router, Lucide icons

---

### Task 1: Remove sibling navigator

**Files:**
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`
- Delete: `apps/web/src/features/chunks/detail/chunk-sibling-navigator.tsx`

- [ ] **Step 1: Remove sibling navigator from the route**

In `apps/web/src/routes/chunks.$chunkId.tsx`, remove the import:

```typescript
import { ChunkSiblingNavigator } from "@/features/chunks/detail/chunk-sibling-navigator";
```

Remove the `ChunkSiblingNavigator` JSX from the flex container (around line 353-357):

```tsx
                    <ChunkSiblingNavigator
                        currentChunkId={chunkId}
                        codebaseId={currentCodebases?.[0]?.id}
                        codebaseName={currentCodebases?.[0]?.name}
                    />
```

- [ ] **Step 2: Remove prev/next keyboard shortcuts**

In the same file, find the keyboard event handler that handles `ArrowUp`/`ArrowDown`/`h`/`l`/`j`/`k` for chunk traversal. Remove those cases from the handler. Also remove the `prevDoc`, `nextDoc`, and `currentIndex` computations if they exist and are only used for navigation.

Search for references to `prevDoc` and `nextDoc` in the file. If they're used nowhere else after removing the keyboard shortcuts, delete their declarations.

- [ ] **Step 3: Delete the sibling navigator component file**

Delete: `apps/web/src/features/chunks/detail/chunk-sibling-navigator.tsx`

- [ ] **Step 4: Run type-check**

Run: `pnpm run check-types`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add -A apps/web/src/routes/chunks.\$chunkId.tsx apps/web/src/features/chunks/detail/chunk-sibling-navigator.tsx
git commit -m "refactor: remove sibling navigator from chunk detail page"
```

---

### Task 2: Move decision context to the Context drawer tab

**Files:**
- Modify: `apps/web/src/features/chunks/detail/chunk-detail-content.tsx`
- Modify: `apps/web/src/features/chunks/detail/more-context-context-tab.tsx`
- Modify: `apps/web/src/features/chunks/detail/more-context-drawer.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

- [ ] **Step 1: Remove the decision context callout from ChunkDetailContent**

In `apps/web/src/features/chunks/detail/chunk-detail-content.tsx`, remove the entire decision context aside block (the `{hasDecisionContext && (...)}` block after the content `<div>`, around lines 121-155).

Also remove the `hasDecisionContext` variable declaration. Remove the `Scale` icon from the lucide imports if it's no longer used. Remove the `rationale`, `alternatives`, `consequences` props from the component's props interface and function signature.

- [ ] **Step 2: Update ChunkDetailContent usage in the route**

In `apps/web/src/routes/chunks.$chunkId.tsx`, remove the `rationale`, `alternatives`, `consequences` props from the `<ChunkDetailContent>` JSX (around lines 370-373):

Remove these lines:
```tsx
                        rationale={rationale}
                        alternatives={alternatives ?? null}
                        consequences={consequences}
```

- [ ] **Step 3: Add decision context props to the drawer**

In `apps/web/src/features/chunks/detail/more-context-drawer.tsx`, add to `MoreContextDrawerProps`:

```typescript
    rationale?: string | null;
    alternatives?: string[] | null;
    consequences?: string | null;
```

Add to the function destructuring and pass to `MoreContextContextTab`:

```tsx
                    {tab === "context" && (
                        <MoreContextContextTab
                            chunkId={chunkId}
                            appliesTo={appliesTo}
                            fileReferences={fileReferences}
                            rationale={rationale}
                            alternatives={alternatives}
                            consequences={consequences}
                        />
                    )}
```

- [ ] **Step 4: Render decision context in the Context tab**

In `apps/web/src/features/chunks/detail/more-context-context-tab.tsx`, add to the props interface:

```typescript
    rationale?: string | null;
    alternatives?: string[] | null;
    consequences?: string | null;
```

Add to the function destructuring.

Add the `Scale` icon import from lucide and `MarkdownRenderer` import:

```typescript
import { Code, FileCode, Scale } from "lucide-react";
import { MarkdownRenderer } from "@/components/markdown-renderer";
```

Add a decision context section at the top of the returned JSX (before the "Applies to" section):

```tsx
            {(rationale || (alternatives && alternatives.length > 0) || consequences) && (
                <section>
                    <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-amber-600 dark:text-amber-400">
                        <Scale className="size-3.5" />
                        Decision context
                    </h3>
                    <div className="rounded-md border-l-2 border-amber-500/40 bg-amber-500/5 px-4 py-3 space-y-3">
                        {rationale && (
                            <div>
                                <div className="mb-1 text-xs font-semibold text-muted-foreground">Rationale</div>
                                <div className="prose prose-sm dark:prose-invert max-w-none text-muted-foreground">
                                    <MarkdownRenderer>{rationale}</MarkdownRenderer>
                                </div>
                            </div>
                        )}
                        {alternatives && alternatives.length > 0 && (
                            <div>
                                <div className="mb-1 text-xs font-semibold text-muted-foreground">Alternatives considered</div>
                                <ul className="text-sm text-muted-foreground list-disc pl-5 space-y-1">
                                    {alternatives.map((alt, i) => (
                                        <li key={i}>{alt}</li>
                                    ))}
                                </ul>
                            </div>
                        )}
                        {consequences && (
                            <div>
                                <div className="mb-1 text-xs font-semibold text-muted-foreground">Consequences</div>
                                <div className="prose prose-sm dark:prose-invert max-w-none text-muted-foreground">
                                    <MarkdownRenderer>{consequences}</MarkdownRenderer>
                                </div>
                            </div>
                        )}
                    </div>
                </section>
            )}
```

- [ ] **Step 5: Pass decision context props to the drawer in the route**

In `apps/web/src/routes/chunks.$chunkId.tsx`, update the `<MoreContextDrawer>` JSX to include the new props:

```tsx
                <MoreContextDrawer
                    open={drawerOpen}
                    onOpenChange={setDrawerOpen}
                    chunkId={chunkId}
                    chunkTitle={chunk.title}
                    outgoing={outgoing}
                    incoming={incoming}
                    appliesTo={appliesTo}
                    fileReferences={fileReferences}
                    initialTab={drawerTab}
                    rationale={rationale}
                    alternatives={alternatives}
                    consequences={consequences}
                />
```

- [ ] **Step 6: Run type-check**

Run: `pnpm run check-types`
Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/features/chunks/detail/chunk-detail-content.tsx apps/web/src/features/chunks/detail/more-context-context-tab.tsx apps/web/src/features/chunks/detail/more-context-drawer.tsx apps/web/src/routes/chunks.\$chunkId.tsx
git commit -m "refactor: move decision context to drawer Context tab"
```

---

### Task 3: Move proposals to the Links drawer tab

**Files:**
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`
- Modify: `apps/web/src/features/chunks/detail/more-context-links-tab.tsx`
- Modify: `apps/web/src/features/chunks/detail/more-context-drawer.tsx`

- [ ] **Step 1: Remove inline ChunkProposalsSection from the route**

In `apps/web/src/routes/chunks.$chunkId.tsx`, remove this JSX from the flex container:

```tsx
                    <ChunkProposalsSection chunkId={chunkId} />
```

Do NOT remove the import — the component will be reused in the Links tab.

- [ ] **Step 2: Add proposals to the Links tab**

In `apps/web/src/features/chunks/detail/more-context-links-tab.tsx`, add the import:

```typescript
import { ChunkProposalsSection } from "@/features/proposals/chunk-proposals-section";
```

Add a proposals section at the end of the returned JSX (after the "Discover" section):

```tsx
            <section>
                <ChunkProposalsSection chunkId={chunkId} />
            </section>
```

The `ChunkProposalsSection` component already handles its own empty state (renders nothing when no proposals exist), so no conditional wrapper needed.

- [ ] **Step 3: Run type-check**

Run: `pnpm run check-types`
Expected: All pass.

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/chunks.\$chunkId.tsx apps/web/src/features/chunks/detail/more-context-links-tab.tsx
git commit -m "refactor: move proposals to drawer Links tab"
```

---

### Task 4: Move feature overlays to the Context drawer tab

**Files:**
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`
- Modify: `apps/web/src/features/chunks/detail/more-context-context-tab.tsx`
- Modify: `apps/web/src/features/chunks/detail/more-context-drawer.tsx`

- [ ] **Step 1: Remove inline FeatureOverlaysSection from the route**

In `apps/web/src/routes/chunks.$chunkId.tsx`, remove this JSX from the flex container:

```tsx
                    <FeatureOverlaysSection
                        deltas={deltas ?? []}
                        appliedFeatures={appliedFeatures ?? []}
                    />
```

Also remove the `FeatureOverlaysSection` function definition (around lines 52-94) from the route file entirely.

- [ ] **Step 2: Add overlay props to the drawer**

In `apps/web/src/features/chunks/detail/more-context-drawer.tsx`, add to `MoreContextDrawerProps`:

```typescript
    deltas?: Array<{
        id: string;
        featureId: string;
        featureName: string;
        featureColor: string | null;
        featureStatus: string;
        delta: Record<string, unknown>;
    }>;
    appliedFeatures?: string[];
```

Add to the function destructuring and pass to `MoreContextContextTab`:

```tsx
                    {tab === "context" && (
                        <MoreContextContextTab
                            chunkId={chunkId}
                            appliesTo={appliesTo}
                            fileReferences={fileReferences}
                            rationale={rationale}
                            alternatives={alternatives}
                            consequences={consequences}
                            deltas={deltas}
                            appliedFeatures={appliedFeatures}
                        />
                    )}
```

- [ ] **Step 3: Render feature overlays in the Context tab**

In `apps/web/src/features/chunks/detail/more-context-context-tab.tsx`, add to the props interface:

```typescript
    deltas?: Array<{
        id: string;
        featureId: string;
        featureName: string;
        featureColor: string | null;
        featureStatus: string;
        delta: Record<string, unknown>;
    }>;
    appliedFeatures?: string[];
```

Add `Layers` to the lucide import:

```typescript
import { Code, FileCode, Layers, Scale } from "lucide-react";
```

Add `Badge` import:

```typescript
import { Badge } from "@/components/ui/badge";
```

Add a feature overlays section at the end of the returned JSX (after "AI enrichment"):

```tsx
            {deltas && deltas.length > 0 && (
                <section>
                    <h3 className="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        <Layers className="size-3.5" />
                        Feature overlays
                    </h3>
                    <div className="space-y-1">
                        {deltas.map(d => (
                            <div key={d.id} className="flex items-center justify-between rounded border px-3 py-2 text-sm">
                                <div className="flex items-center gap-2">
                                    <span
                                        className="size-2 rounded-full"
                                        style={{ backgroundColor: d.featureColor ?? "#8b5cf6" }}
                                    />
                                    <span className="font-medium">{d.featureName}</span>
                                    {appliedFeatures?.includes(d.featureId) && (
                                        <Badge variant="secondary" size="sm">active</Badge>
                                    )}
                                </div>
                                <span className="text-xs text-muted-foreground">
                                    {Object.keys(d.delta).join(", ")}
                                </span>
                            </div>
                        ))}
                    </div>
                </section>
            )}
```

- [ ] **Step 4: Pass overlay props to the drawer in the route**

In `apps/web/src/routes/chunks.$chunkId.tsx`, update the `<MoreContextDrawer>` JSX to include:

```tsx
                    deltas={deltas}
                    appliedFeatures={appliedFeatures}
```

- [ ] **Step 5: Clean up unused imports in the route**

In the route file, remove the `ChunkProposalsSection` import if it's no longer used directly (it was moved to the Links tab in Task 3). Check if `ChunkSiblingNavigator` import was already removed in Task 1.

- [ ] **Step 6: Run type-check**

Run: `pnpm run check-types`
Expected: All pass.

- [ ] **Step 7: Run full test suite**

Run: `pnpm test`
Expected: All pass.

- [ ] **Step 8: Commit**

```bash
git add apps/web/src/routes/chunks.\$chunkId.tsx apps/web/src/features/chunks/detail/more-context-context-tab.tsx apps/web/src/features/chunks/detail/more-context-drawer.tsx
git commit -m "refactor: move feature overlays to drawer Context tab"
```

---

### Task 5: Final verification

**Files:** None (verification only)

- [ ] **Step 1: Run type-check and tests**

Run: `pnpm run check-types && pnpm test`
Expected: All pass.

- [ ] **Step 2: Manual browser verification**

Run: `pnpm dev`

Verify:
1. Chunk detail page shows 2-column layout (content + right metadata panel). No left sidebar.
2. No sibling navigator visible. h/l/j/k keyboard shortcuts don't navigate between chunks.
3. Decision context callout is NOT in the main content area.
4. Open drawer → Context tab: decision context appears at top with amber styling (only if chunk has rationale/alternatives/consequences).
5. Open drawer → Context tab: feature overlays appear at bottom (only if chunk has deltas).
6. Open drawer → Links tab: proposals section appears after Discover (only if chunk has proposals).
7. Keyboard shortcut `m` still opens the drawer. Badge count unchanged.
8. Page works on mobile (metadata panel hidden, drawer still functional).

- [ ] **Step 3: Commit any fixes**

If verification reveals issues, fix and commit individually.
