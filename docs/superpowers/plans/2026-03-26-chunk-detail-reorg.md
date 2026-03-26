# Chunk Detail Reorganization Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorganize the chunk detail page from a 12-section vertical scroll into collapsible sections for better navigation.

**Architecture:** Convert existing sections into an accordion-style layout where primary content (tags, content, decision context) is always visible, and secondary sections (connections, comments, AI tools, version history, suggested/related) are collapsible. Uses the existing Collapsible component from shadcn/base-ui.

**Tech Stack:** React, Tailwind CSS, shadcn-ui (base-ui Collapsible)

**Codebase notes:**
- Chunk detail is at `apps/web/src/routes/chunks.$chunkId.tsx` (lines 207-559)
- 12 sections separated by 9 `<Separator>` elements
- Sections: Header → Metadata → Tags → Content → AppliesTo → FileRefs → DecisionContext → AI → Comments → Connections → Suggested → Related → VersionHistory

---

## File Structure

### New files:
- `apps/web/src/features/chunks/collapsible-section.tsx` — Reusable collapsible section component

### Files to modify:
- `apps/web/src/routes/chunks.$chunkId.tsx` — Wrap secondary sections in collapsible wrappers

---

## Task 1: Create CollapsibleSection Component

**Files:**
- Create: `apps/web/src/features/chunks/collapsible-section.tsx`

- [ ] **Step 1: Check if Collapsible component exists**

Run: `ls apps/web/src/components/ui/collapsible.tsx`
If not, check for an accordion or disclosure component. If none exist, create a simple collapsible using useState + CSS transition.

- [ ] **Step 2: Create CollapsibleSection**

```tsx
// apps/web/src/features/chunks/collapsible-section.tsx
import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { LucideIcon } from "lucide-react";

interface CollapsibleSectionProps {
    title: string;
    icon?: LucideIcon;
    count?: number;
    defaultOpen?: boolean;
    children: React.ReactNode;
}

export function CollapsibleSection({ title, icon: Icon, count, defaultOpen = false, children }: CollapsibleSectionProps) {
    const [open, setOpen] = useState(defaultOpen);

    return (
        <div className="border-t py-3">
            <button
                onClick={() => setOpen(!open)}
                className="flex w-full items-center gap-2 text-sm font-medium hover:text-foreground text-muted-foreground transition-colors"
            >
                {open ? <ChevronDown className="size-4" /> : <ChevronRight className="size-4" />}
                {Icon && <Icon className="size-4" />}
                {title}
                {count != null && (
                    <span className="ml-auto rounded-full bg-muted px-2 py-0.5 text-xs">{count}</span>
                )}
            </button>
            {open && <div className="mt-3">{children}</div>}
        </div>
    );
}
```

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(web): add CollapsibleSection component"
```

---

## Task 2: Wrap Secondary Sections in Collapsibles

**Files:**
- Modify: `apps/web/src/routes/chunks.$chunkId.tsx`

- [ ] **Step 1: Read the full component**

Understand the 12 sections and identify which are primary (always visible) vs secondary (collapsible).

**Primary (always visible):**
- Header with actions
- Metadata + health badge
- Tags (inline editor)
- Content (markdown)
- Decision Context (if present)

**Secondary (collapsible, default closed):**
- Applies To (default open if has items)
- File References (default open if has items)
- AI Section (default closed)
- Comments (default closed)
- Connections (default open)
- Suggested Connections (default closed)
- Related Chunks (default closed)
- Version History (default closed)

- [ ] **Step 2: Wrap each secondary section**

Replace the `<Separator>` + section pattern with `<CollapsibleSection>`:

```tsx
// Before:
<Separator />
<div>
    <h3>Connections</h3>
    {/* connection content */}
</div>

// After:
<CollapsibleSection
    title="Connections"
    icon={Network}
    count={connections.length}
    defaultOpen={connections.length > 0}
>
    {/* connection content */}
</CollapsibleSection>
```

Apply to each secondary section:
- AppliesTo: `title="Applies To"`, `icon={Code}` (existing codebase uses `Code` icon, NOT `FolderTree`), `count={appliesTo.length}`, `defaultOpen={appliesTo.length > 0}`
- FileRefs: `title="File References"`, `icon={FileCode}`, `count={fileReferences.length}`, `defaultOpen={fileReferences.length > 0}`
- AI Section: `title="AI Tools"`, `icon={Sparkles}`, `defaultOpen={false}`
- Comments: `title="Comments"`, `icon={MessageSquare}`, `defaultOpen={false}` — **NOTE: ChunkComments already has its own expand/collapse toggle. Strip that internal toggle when wrapping, or the UI will have double-nested collapsibles.**
- Connections: `title="Connections"`, `icon={Network}`, `count={connections.length}`, `defaultOpen={true}`
- Suggested: `title="Suggested Connections"`, `icon={Lightbulb}`, `defaultOpen={false}`
- Related: `title="Related Chunks"`, `icon={LinkIcon}`, `defaultOpen={false}`
- Version History: `title="Version History"`, `icon={History}`, `defaultOpen={false}`

Remove the standalone `<Separator>` elements that separated these sections — the `CollapsibleSection` border-t handles visual separation.

- [ ] **Step 3: Add visual hierarchy to connections**

Inside the Connections section, distinguish outgoing vs incoming with directional indicators:

```tsx
<div className="space-y-3">
    <div>
        <h4 className="text-xs font-medium text-muted-foreground mb-2 flex items-center gap-1">
            <ArrowRight className="size-3" /> Links to ({outgoing.length})
        </h4>
        {/* outgoing connections */}
    </div>
    <div>
        <h4 className="text-xs font-medium text-muted-foreground mb-2 flex items-center gap-1">
            <ArrowLeft className="size-3" /> Linked from ({incoming.length})
        </h4>
        {/* incoming connections */}
    </div>
</div>
```

- [ ] **Step 4: Commit**

```bash
git commit -m "feat(web): reorganize chunk detail into collapsible sections"
```
