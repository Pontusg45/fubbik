# Landing Background Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the busy labeled-node canvas background on the landing page with a subtle monochrome constellation of dots and faint edges.

**Architecture:** Single-file rewrite of `KnowledgeGraphCanvas`. Same canvas-based approach, same component API, same container in `index.tsx`. The draw loop changes from labeled rectangles to simple circles + thin lines with breathing opacity.

**Tech Stack:** React, Canvas 2D API

---

### Task 1: Rewrite KnowledgeGraphCanvas with constellation dots and edges

**Files:**
- Modify: `apps/web/src/features/landing/knowledge-graph-canvas.tsx` (full rewrite)

- [ ] **Step 1: Replace the file contents with the new constellation implementation**

```tsx
import { useCallback, useEffect, useRef } from "react";

/* ── Constellation data ── */

const INDIGO: [number, number, number] = [129, 140, 248]; // #818cf8
const SLATE: [number, number, number] = [148, 163, 184]; // #94a3b8

// 10 primary dots + 5 accent dots
// [relativeX (0-1), relativeY (0-1), radius, color, baseOpacity, breathDuration]
const DOT_TEMPLATES: Array<
  [number, number, number, [number, number, number], number, number]
> = [
  // Primary dots (r 1.5–2.5)
  [0.12, 0.18, 2.0, INDIGO, 0.30, 4.0],
  [0.28, 0.32, 2.5, INDIGO, 0.25, 5.5],
  [0.45, 0.22, 1.8, SLATE, 0.30, 4.5],
  [0.65, 0.35, 2.2, INDIGO, 0.25, 6.0],
  [0.82, 0.20, 1.5, SLATE, 0.25, 5.0],
  [0.20, 0.55, 1.8, SLATE, 0.20, 7.0],
  [0.38, 0.48, 2.0, INDIGO, 0.22, 5.2],
  [0.55, 0.58, 1.5, SLATE, 0.18, 6.5],
  [0.72, 0.50, 2.3, INDIGO, 0.20, 8.0],
  [0.90, 0.45, 1.5, SLATE, 0.18, 6.0],
  // Accent dots (r 0.6–0.9)
  [0.08, 0.40, 0.8, INDIGO, 0.10, 9.0],
  [0.52, 0.12, 0.6, SLATE, 0.10, 7.2],
  [0.95, 0.30, 0.7, INDIGO, 0.08, 8.5],
  [0.35, 0.70, 0.9, SLATE, 0.08, 10.0],
  [0.78, 0.65, 0.7, INDIGO, 0.07, 11.0],
];

// Edges between primary dots (indices into DOT_TEMPLATES, 0–9)
const EDGE_PAIRS: Array<[number, number, number]> = [
  // [sourceIdx, targetIdx, breathDuration]
  [0, 1, 7.0],
  [1, 2, 5.5],
  [2, 3, 6.0],
  [3, 4, 8.0],
  [4, 9, 6.5],
  [5, 6, 7.5],
  [6, 7, 6.0],
  [7, 8, 5.0],
  [8, 9, 8.0],
  [1, 6, 9.0],
  [3, 8, 7.0],
  [0, 5, 10.0],
];

/* ── Types ── */

interface Dot {
  x: number;
  y: number;
  vx: number;
  vy: number;
  r: number;
  color: [number, number, number];
  baseOpacity: number;
  breathDuration: number;
  breathOffset: number;
}

/* ── Helpers ── */

function createDots(w: number, h: number): Dot[] {
  return DOT_TEMPLATES.map(([rx, ry, r, color, baseOpacity, breathDuration]) => ({
    x: rx * w,
    y: ry * h,
    vx: (Math.random() - 0.5) * 0.15,
    vy: (Math.random() - 0.5) * 0.15,
    r,
    color,
    baseOpacity,
    breathDuration,
    breathOffset: Math.random() * Math.PI * 2,
  }));
}

/* ── Component ── */

export function KnowledgeGraphCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const dotsRef = useRef<Dot[]>([]);
  const mouseRef = useRef<{ x: number; y: number } | null>(null);
  const rafRef = useRef<number>(0);
  const reducedMotionRef = useRef(false);
  const startTimeRef = useRef(performance.now());

  const init = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);

    dotsRef.current = createDots(rect.width, rect.height);
  }, []);

  useEffect(() => {
    const mq = window.matchMedia("(prefers-reduced-motion: reduce)");
    reducedMotionRef.current = mq.matches;
    const handler = (e: MediaQueryListEvent) => {
      reducedMotionRef.current = e.matches;
    };
    mq.addEventListener("change", handler);

    init();

    const onResize = () => init();
    window.addEventListener("resize", onResize);

    const canvas = canvasRef.current;
    const onMouseMove = (e: MouseEvent) => {
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      mouseRef.current = { x: e.clientX - rect.left, y: e.clientY - rect.top };
    };
    const onMouseLeave = () => {
      mouseRef.current = null;
    };

    canvas?.addEventListener("mousemove", onMouseMove);
    canvas?.addEventListener("mouseleave", onMouseLeave);

    const animate = () => {
      const cvs = canvasRef.current;
      if (!cvs) return;
      const ctx = cvs.getContext("2d");
      if (!ctx) return;

      const dpr = window.devicePixelRatio || 1;
      const w = cvs.width / dpr;
      const h = cvs.height / dpr;
      const dots = dotsRef.current;
      const mouse = mouseRef.current;
      const elapsed = (performance.now() - startTimeRef.current) / 1000;

      ctx.clearRect(0, 0, w, h);

      // Ambient center glow
      const glow = ctx.createRadialGradient(w * 0.5, h * 0.35, 0, w * 0.5, h * 0.35, w * 0.45);
      glow.addColorStop(0, "rgba(99, 102, 241, 0.04)");
      glow.addColorStop(1, "rgba(99, 102, 241, 0)");
      ctx.fillStyle = glow;
      ctx.fillRect(0, 0, w, h);

      // Mouse glow
      if (mouse) {
        const mg = ctx.createRadialGradient(mouse.x, mouse.y, 0, mouse.x, mouse.y, 200);
        mg.addColorStop(0, "rgba(129, 140, 248, 0.06)");
        mg.addColorStop(1, "rgba(129, 140, 248, 0)");
        ctx.fillStyle = mg;
        ctx.fillRect(0, 0, w, h);
      }

      // Update positions (skip if reduced motion)
      if (!reducedMotionRef.current) {
        for (const dot of dots) {
          dot.x += dot.vx;
          dot.y += dot.vy;

          if (dot.x - dot.r <= 0) {
            dot.x = dot.r;
            dot.vx = Math.abs(dot.vx);
          } else if (dot.x + dot.r >= w) {
            dot.x = w - dot.r;
            dot.vx = -Math.abs(dot.vx);
          }

          if (dot.y - dot.r <= 0) {
            dot.y = dot.r;
            dot.vy = Math.abs(dot.vy);
          } else if (dot.y + dot.r >= h) {
            dot.y = h - dot.r;
            dot.vy = -Math.abs(dot.vy);
          }
        }
      }

      // Compute dot opacities (breathing + mouse proximity)
      const dotOpacities = dots.map((dot) => {
        const breath = Math.sin(elapsed * (Math.PI * 2) / dot.breathDuration + dot.breathOffset);
        const breathAlpha = dot.baseOpacity + breath * dot.baseOpacity * 0.5;

        let alpha = breathAlpha;
        if (mouse) {
          const dx = mouse.x - dot.x;
          const dy = mouse.y - dot.y;
          const dist = Math.sqrt(dx * dx + dy * dy);
          if (dist < 200) {
            const proximity = 1 - dist / 200;
            alpha = Math.min(0.6, alpha + proximity * (0.6 - alpha));
          }
        }

        return alpha;
      });

      // Draw edges
      for (const [si, ti, breathDur] of EDGE_PAIRS) {
        const s = dots[si];
        const t = dots[ti];
        if (!s || !t) continue;

        const breath = Math.sin(elapsed * (Math.PI * 2) / breathDur + si);
        const baseAlpha = 0.04 + breath * 0.02;

        // Brighten edge if either connected dot is brightened by mouse
        const edgeAlpha = Math.max(baseAlpha, Math.min(dotOpacities[si], dotOpacities[ti]) * 0.3);

        ctx.beginPath();
        ctx.moveTo(s.x, s.y);
        ctx.lineTo(t.x, t.y);
        ctx.strokeStyle = `rgba(148, 163, 184, ${edgeAlpha})`;
        ctx.lineWidth = 0.5;
        ctx.stroke();
      }

      // Draw dots
      for (let i = 0; i < dots.length; i++) {
        const dot = dots[i];
        const alpha = dotOpacities[i];
        const [r, g, b] = dot.color;

        ctx.beginPath();
        ctx.arc(dot.x, dot.y, dot.r, 0, Math.PI * 2);
        ctx.fillStyle = `rgba(${r}, ${g}, ${b}, ${alpha})`;
        ctx.fill();
      }

      rafRef.current = requestAnimationFrame(animate);
    };

    rafRef.current = requestAnimationFrame(animate);

    return () => {
      cancelAnimationFrame(rafRef.current);
      window.removeEventListener("resize", onResize);
      canvas?.removeEventListener("mousemove", onMouseMove);
      canvas?.removeEventListener("mouseleave", onMouseLeave);
      mq.removeEventListener("change", handler);
    };
  }, [init]);

  return (
    <canvas
      ref={canvasRef}
      className="pointer-events-auto absolute inset-0 size-full"
    />
  );
}
```

- [ ] **Step 2: Verify the dev server compiles without errors**

Run: `cd /Users/pontus/projects/fubbik && pnpm dev`

Expected: No TypeScript or build errors. The landing page at `http://localhost:3001` shows subtle monochrome dots with faint edges instead of the old labeled nodes.

- [ ] **Step 3: Visual smoke test**

Check in browser:
1. Dots drift slowly and bounce off edges
2. Moving mouse over canvas brightens nearby dots and edges
3. Moving mouse away returns dots to ambient state
4. Bottom gradient still fades content cleanly
5. Hero text is clearly readable without background competing

- [ ] **Step 4: Test reduced motion**

In browser DevTools → Rendering → check "Emulate CSS prefers-reduced-motion: reduce".

Expected: Dots stop drifting but breathing continues. No JS errors.

- [ ] **Step 5: Test window resize**

Resize the browser window.

Expected: Canvas re-initializes, dots reposition proportionally. No visual glitches.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/features/landing/knowledge-graph-canvas.tsx
git commit -m "feat(landing): replace busy node canvas with soft constellation background

Monochrome dots with faint edges hint at a graph without competing
with hero content. Breathing opacity, slow drift, mouse proximity
brightening."
```
