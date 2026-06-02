# Dashboard Redesign — Focus Stream

## Problem

The current dashboard has 12 widgets, 9 API queries, and a 2000+ pixel scroll depth. The right sidebar alone has 8 stacked cards with equal
visual weight. There's no information hierarchy — knowledge health, featured chunk, smart collections, and activity feed all compete for
attention equally.

## Goal

Replace the cluttered multi-widget dashboard with a single-column "Focus Stream" layout: a compact stats bar, an active plan focus card, and
a unified chronological feed that interleaves proposals, staleness alerts, and activity. Reduce from 12 widgets / 770 lines to 3 zones /
~250 lines.

---

## 1. Layout

Single column, `max-w-3xl` (~800px), centered. Three zones stacked vertically.

### Zone 1 — Stats Bar

Compact inline row showing key numbers:

```
31 chunks · 47 connections · 7 requirements · 2 pending proposals · 3 stale
```

- Proposals and stale counts are amber-colored when > 0, acting as subtle alerts
- Data sources: `GET /api/stats`, `GET /api/proposals/count`, `GET /api/chunks/stale/count`
- Always visible, takes one line of height

### Zone 2 — Active Plan Card

Indigo-tinted card showing the current in-progress plan:

- **Selection logic:** first plan with `status=in_progress`. If none, first with `status=ready`. If none, collapses to a one-line "No active
  plan — Start one →" link.
- **Card contents:** plan title, status pill, task progress counter (`2/5`), thin progress bar, and task checklist (max 8 tasks visible,
  "and N more" if overflow)
- **Interactive:** task checkboxes toggle `pending ↔ done` via `PATCH /api/plans/:id/tasks/:taskId`. On toggle, refetch the plan query.
- **Data:** `GET /api/plans?status=in_progress` (fallback: `GET /api/plans?status=ready`)

### Zone 3 — Unified Feed

Reverse-chronological feed interleaving three item types:

**Item types:**

| Type               | Dot color      | Badge                             | Content              | Actions                         |
| ------------------ | -------------- | --------------------------------- | -------------------- | ------------------------------- |
| Proposal (pending) | Amber (bright) | `PROPOSAL` amber bg               | Chunk title + reason | Approve / Reject buttons inline |
| Staleness flag     | Amber (muted)  | `STALE` amber bg                  | Chunk title + reason | Click navigates to chunk        |
| Activity           | Gray           | `CREATED` / `UPDATED` / `DELETED` | Entity title + type  | Click navigates to entity       |

**Filter tabs** above the feed: `All | Proposals | Stale | Activity`. Default: `All`. Tabs are inclusive filters — selecting "Proposals"
shows only proposal items. Selecting "All" shows everything.

**Data sources (merged client-side):**

- `GET /api/proposals?status=pending` — all pending proposals
- `GET /api/chunks/stale?limit=10` — undismissed staleness flags
- `GET /api/activity?limit=20` — recent activity log

All three responses are merged into one array, sorted by timestamp (descending), and rendered as a flat list.

**Inline proposal actions:** Approve/Reject buttons call `POST /api/proposals/:id/approve` and `POST /api/proposals/:id/reject` directly
from the feed. On success, the item transitions to a "Approved ✓" / "Rejected" state and fades out after 2 seconds.

**Pagination:** "Load more" button at the bottom fetches the next 20 activity items. Proposals and stale flags are always fully loaded
(bounded — typically < 10 items each).

---

## 2. Empty States

| Condition                                      | Behavior                                                                            |
| ---------------------------------------------- | ----------------------------------------------------------------------------------- |
| No active plan (no in_progress or ready plans) | Zone 2 collapses to: "No active plan — [Start one →](/plans/new)"                   |
| Plan with 0 tasks                              | Shows plan title + status pill, no checklist. Link: "+ Add task"                    |
| Empty feed (all three sources empty)           | "Nothing happening yet. Create your first chunk or plan to get started." with links |
| Feed with only one type                        | Filter tabs still visible, empty types show 0 items                                 |

---

## 3. Mobile

Single column is already mobile-friendly. Stats bar wraps to 2 rows on small screens. No layout changes needed.

---

## 4. Files Changed

### Rewritten

| Path                                | Change                                                                                    |
| ----------------------------------- | ----------------------------------------------------------------------------------------- |
| `apps/web/src/routes/dashboard.tsx` | Complete rewrite — 770 lines → ~250 lines. Imports StatsBar, ActivePlanCard, UnifiedFeed. |

### Created

| Path                                                   | Responsibility                                                         |
| ------------------------------------------------------ | ---------------------------------------------------------------------- |
| `apps/web/src/features/dashboard/stats-bar.tsx`        | Compact inline stats row with amber highlights                         |
| `apps/web/src/features/dashboard/active-plan-card.tsx` | Plan focus card with interactive task checklist                        |
| `apps/web/src/features/dashboard/unified-feed.tsx`     | Merged feed with filter tabs                                           |
| `apps/web/src/features/dashboard/feed-item.tsx`        | Single feed item — type-specific rendering for proposal/stale/activity |

### Deleted

| Path                                                        | Reason                                         |
| ----------------------------------------------------------- | ---------------------------------------------- |
| `apps/web/src/features/dashboard/featured-chunk-widget.tsx` | Cut — low value                                |
| `apps/web/src/features/dashboard/smart-collections.tsx`     | Cut — static UI, low usage                     |
| `apps/web/src/features/dashboard/missed-chunks-widget.tsx`  | Cut — low value                                |
| `apps/web/src/features/dashboard/milestone-cards.tsx`       | Cut — clutters the page                        |
| `apps/web/src/features/dashboard/welcome-wizard.tsx`        | Cut — can return as a dismissible banner later |
| `apps/web/src/features/dashboard/attention-needed.tsx`      | Absorbed into unified feed                     |

### Unchanged

- All API endpoints — no backend changes
- All other web pages — untouched
- Nav, header, breadcrumbs — untouched

---

## 5. Out of Scope

- **Welcome wizard** — removed. Can return as a dismissible banner above stats bar in a follow-up.
- **Favorites / recently viewed** — accessible via nav sidebar components (`RecentlyViewed`, `ReadingTrailSidebar`). Not on the dashboard.
- **Knowledge health summary** — lives at `/knowledge-health`.
- **Requirements summary** — lives at `/requirements`.
- **Featured chunk / smart collections / missed chunks** — cut entirely.
- **Drag-to-reorder tasks in the plan card** — interactive checkboxes only.
- **Real-time feed updates** — no WebSocket/polling. Refetch on focus or manual refresh.
- **Customizable dashboard** — no user-configurable widget layout.

---

## Success Criteria

- Dashboard renders 3 zones: stats bar, active plan card, unified feed
- Stats bar shows chunk/connection/requirement counts + amber proposal/stale counts
- Active plan card shows the first in_progress (or ready) plan with interactive task checkboxes
- No active plan → one-line fallback with link
- Feed interleaves proposals, staleness, and activity in reverse chronological order
- Filter tabs (All / Proposals / Stale / Activity) work
- Proposal items have inline Approve / Reject buttons that call the API
- "Load more" at the bottom fetches more activity items
- Old dashboard widgets (12) are replaced — featured chunk, milestones, smart collections, etc. are gone
- Page loads with 3 API queries (stats, plan, feed sources) instead of 9
- Total component code is ~250 lines (down from 770)
