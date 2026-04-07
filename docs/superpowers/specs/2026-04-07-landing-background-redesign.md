# Landing Page Background Redesign — Soft Constellation

## Problem

The current `KnowledgeGraphCanvas` background on the landing page is too busy. Labeled nodes with colored borders, text, and dashed edges compete with the hero content for attention. The background should support the content, not fight it.

## Design

Replace the current labeled-node canvas animation with a **Soft Constellation** — abstract monochrome dots connected by barely-visible lines. No labels, no borders, no fills. The result hints at a knowledge graph without spelling it out.

### Visual Specification

**Dots:**
- ~10 primary dots, radius 1.5–2.5px
- ~5 accent dots, radius 0.6–0.9px (scattered fill)
- Monochrome palette: indigo-400 (`#818cf8`) and slate-400 (`#94a3b8`), alternating
- Base opacity: 0.15–0.35 for primary, 0.08–0.12 for accent
- Breathing animation: each dot pulses opacity over 4–11s (staggered, no two share a duration)

**Edges:**
- ~12 connection lines between primary dots
- 0.5px stroke, slate-400 at 3–8% opacity
- Breathing animation on opacity (5–10s cycles, staggered)

**Ambient glow:**
- Centered radial gradient, indigo at ~4% opacity, elliptical, covering upper-center area

**Bottom fade:**
- Linear gradient from background color upward, covering bottom 45% of canvas (unchanged from current)

### Animation

**Drift:** Dots move at ~0.15px/frame (half the current 0.3px/frame). Edges follow their connected dots. Bounce off canvas edges.

**Breathing:** Independent opacity oscillation per dot and per edge. Staggered durations prevent synchronized pulsing.

**Mouse proximity:** Dots within a 200px radius of the cursor brighten to ~0.6 opacity (smooth transition). Edges connected to brightened dots also brighten proportionally. No other mouse interaction — no repulsion, no click behavior.

**Reduced motion:** When `prefers-reduced-motion: reduce` is active, skip all position updates (dots stay at initial positions). Breathing animation is CSS-driven and inherits the media query naturally.

### Implementation

**File:** Replace the contents of `apps/web/src/features/landing/knowledge-graph-canvas.tsx`.

**Approach:** Keep the existing canvas-based architecture. The component already handles:
- DPR-aware canvas sizing
- Resize listener
- Mouse tracking
- `requestAnimationFrame` loop
- Reduced motion detection

**Changes:**
1. Remove `NODE_LABELS`, `NODE_TYPES`, `TYPE_COLORS`, `EDGE_TEMPLATES` constants
2. Replace with dot position/size/opacity data and edge index pairs
3. Simplify `createNodes` → `createDots` (no text measurement needed)
4. Remove all text rendering from the draw loop
5. Remove node rectangle rendering — draw circles only
6. Simplify edge rendering — no labels, no dash toggling, just faint lines
7. Adjust mouse proximity: brighten dot opacity, no edge label reveal
8. Halve velocity constants (0.3 → 0.15)
9. Add per-dot and per-edge breathing phase (using `Math.sin` on elapsed time with per-element frequency)

**No changes to:**
- `apps/web/src/routes/index.tsx` (the background container and gradient overlay stay the same)
- Canvas element class names or positioning
- The component's public API (still a zero-prop `<KnowledgeGraphCanvas />`)

### Performance

The new version is strictly lighter than the current one:
- No `ctx.measureText` calls
- No `ctx.fillText` calls
- No `ctx.roundRect` + fill + stroke per node (replaced with single `ctx.arc` + fill)
- Fewer elements overall (~15 dots vs 14 labeled nodes)
- Simpler edge drawing (no dash state toggling, no label rendering)

## Out of Scope

- Changing the bottom gradient or hero section layout
- Adding WebGL or shader-based effects
- Changing the background on the `/features` page
- Dark/light mode toggling (landing page is dark-only)
