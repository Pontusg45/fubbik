# Visual Polish Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix visual inconsistencies: theme-safe colors, plan action cleanup, step completion styling, consistent form inputs, notification accessibility.

**Architecture:** Mostly CSS/Tailwind changes with some component restructuring. No backend changes. Each task is independent.

**Tech Stack:** React, Tailwind CSS, shadcn-ui (base-ui)

---

## File Structure

### Files to modify:
- `apps/web/src/routes/requirements_.$requirementId.tsx` — Replace hardcoded status colors
- `apps/web/src/routes/plans.$planId.tsx` — Cleanup action buttons into dropdown
- `apps/web/src/features/plans/plan-step-item.tsx` — Add completion visual feedback
- `apps/web/src/features/nav/notification-bell.tsx` — Fix delete button visibility
- `apps/web/src/routes/chunks.new.tsx` — Standardize form inputs
- `apps/web/src/routes/chunks.$chunkId_.edit.tsx` — Standardize form inputs
- `apps/web/src/routes/templates.tsx` — Standardize overlay backdrop

---

## Task 1: Theme-Safe Status Colors

**Files:**
- Modify: `apps/web/src/routes/requirements_.$requirementId.tsx`

- [ ] **Step 1: Read the file**

Find the status button styling at lines 347-355 where `bg-emerald-500 text-white` and `bg-red-500 text-white` are hardcoded.

- [ ] **Step 2: Replace with theme-aware variants**

The `/10` opacity versions at lines 35-39 are fine (they work in both themes). The issue is the **full solid** colors on the active state buttons. Replace:

```tsx
// Before:
? "bg-emerald-500 text-white"
// After:
? "bg-emerald-500/90 text-white dark:bg-emerald-600"

// Before:
? "bg-red-500 text-white"
// After:
? "bg-red-500/90 text-white dark:bg-red-600"
```

This keeps the visual pop while being slightly more theme-aware.

- [ ] **Step 3: Commit**

```bash
git commit -m "fix(web): improve status color contrast in dark mode"
```

---

## Task 2: Plan Action Buttons Cleanup

**Files:**
- Modify: `apps/web/src/routes/plans.$planId.tsx`

- [ ] **Step 1: Read the actions section**

Find lines 254-314 with 4 conditional status buttons + 1 delete button.

- [ ] **Step 2: Replace with primary action + dropdown**

Keep the most logical next action as a standalone button. Move the rest into a dropdown:

```tsx
<div className="flex items-center gap-2">
    {/* Primary action: the logical next step */}
    {plan.status === "draft" && (
        <Button size="sm" onClick={() => updateStatusMutation.mutate("active")}>
            <Play className="mr-1.5 size-3.5" /> Activate Plan
        </Button>
    )}
    {plan.status === "active" && (
        <Button size="sm" onClick={() => updateStatusMutation.mutate("completed")}>
            <CheckCircle className="mr-1.5 size-3.5" /> Mark Complete
        </Button>
    )}
    {(plan.status === "completed" || plan.status === "archived") && (
        <Button size="sm" variant="outline" onClick={() => updateStatusMutation.mutate("active")}>
            <RotateCcw className="mr-1.5 size-3.5" /> Reactivate
        </Button>
    )}

    {/* Overflow menu for secondary actions */}
    <DropdownMenu>
        <DropdownMenuTrigger render={<Button variant="outline" size="sm" />}>
            <MoreHorizontal className="size-4" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
            {plan.status !== "draft" && (
                <DropdownMenuItem onClick={() => updateStatusMutation.mutate("draft")}>
                    Move to Draft
                </DropdownMenuItem>
            )}
            {plan.status !== "archived" && (
                <DropdownMenuItem onClick={() => updateStatusMutation.mutate("archived")}>
                    Archive
                </DropdownMenuItem>
            )}
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => setShowDeleteDialog(true)} className="text-destructive">
                Delete Plan
            </DropdownMenuItem>
        </DropdownMenuContent>
    </DropdownMenu>
</div>
```

Read the existing DropdownMenu pattern from `__root.tsx` or `chunk-row-actions.tsx` to match the base-ui API.

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): simplify plan actions with primary button + overflow menu"
```

---

## Task 3: Step Completion Visual Feedback

**Files:**
- Modify: `apps/web/src/features/plans/plan-step-item.tsx`

- [ ] **Step 1: Read the component**

Find how completed, blocked, and in-progress steps are currently styled.

- [ ] **Step 2: Enhance status styling**

The component currently uses `border-b last:border-b-0` on the wrapper div (NOT `border-l-2` — that was an error). Replace the bottom border with a left border for status indication:

```tsx
// Update the wrapper div's border-left styling:
const borderColor = {
    done: "border-green-500/50 bg-green-500/5",
    in_progress: "border-blue-500/50 bg-blue-500/5",
    blocked: "border-red-500/50 bg-red-500/5",
    skipped: "border-muted bg-muted/20",
    pending: "border-muted",
}[step.status] ?? "border-muted";

// Update the description text:
const textStyle = step.status === "done"
    ? "line-through text-muted-foreground"
    : step.status === "blocked"
    ? "text-red-600 dark:text-red-400"
    : "";
```

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): enhance plan step visual feedback with status colors"
```

---

## Task 4: Notification Delete Button Accessibility

**Files:**
- Modify: `apps/web/src/features/nav/notification-bell.tsx`

- [ ] **Step 1: Find the opacity-0 button**

At line 147: `className="shrink-0 opacity-0 transition-opacity group-hover:opacity-100"`

- [ ] **Step 2: Replace with always-visible, muted styling**

```tsx
// Before:
className="shrink-0 opacity-0 transition-opacity group-hover:opacity-100 hover:opacity-100 focus:opacity-100"

// After:
className="shrink-0 text-muted-foreground/50 hover:text-destructive transition-colors"
```

This makes the button always visible but subtle, becoming red on hover. Works on mobile and for keyboard users.

- [ ] **Step 3: Commit**

```bash
git commit -m "fix(web): make notification delete button always visible for accessibility"
```

---

## Task 5: Consistent Form Inputs

**Files:**
- Modify: `apps/web/src/routes/chunks.new.tsx`
- Modify: `apps/web/src/routes/chunks.$chunkId_.edit.tsx`

- [ ] **Step 1: Find raw input elements**

Search for raw `<input` elements with inline className styling instead of the `<Input />` component. These look like:

```tsx
<input className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none" />
```

- [ ] **Step 2: Replace with `<Input />` component**

Import `Input` from `@/components/ui/input` and replace raw inputs:

```tsx
// Before:
<input className="bg-background focus:ring-ring w-full rounded-md border px-3 py-2 text-sm ..." />

// After:
<Input ... />
```

The `<Input>` component already includes all the necessary styling.

- [ ] **Step 3: Fix template overlay backdrop**

In `apps/web/src/routes/templates.tsx`, find `bg-black/50` overlay and replace with:
```tsx
className="bg-background/80 backdrop-blur-sm"
```

- [ ] **Step 4: Commit**

```bash
git commit -m "fix(web): standardize form inputs and overlay backdrops"
```
