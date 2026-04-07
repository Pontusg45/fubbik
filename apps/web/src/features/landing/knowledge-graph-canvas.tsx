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
  [0.78, 0.65, 0.7, INDIGO, 0.08, 11.0],
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

function createDots(w: number, h: number, existing?: Dot[]): Dot[] {
  return DOT_TEMPLATES.map(([rx, ry, r, color, baseOpacity, breathDuration], i) => ({
    x: rx * w,
    y: ry * h,
    vx: existing?.[i]?.vx ?? (Math.random() - 0.5) * 0.15,
    vy: existing?.[i]?.vy ?? (Math.random() - 0.5) * 0.15,
    r,
    color,
    baseOpacity,
    breathDuration,
    breathOffset: existing?.[i]?.breathOffset ?? Math.random() * Math.PI * 2,
  }));
}

/* ── Component ── */

export function KnowledgeGraphCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const ctxRef = useRef<CanvasRenderingContext2D | null>(null);
  const sizeRef = useRef({ w: 0, h: 0 });
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

    ctxRef.current = ctx;
    sizeRef.current = { w: rect.width, h: rect.height };
    dotsRef.current = createDots(rect.width, rect.height, dotsRef.current);
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
      const ctx = ctxRef.current;
      if (!ctx) return;
      const { w, h } = sizeRef.current;
      if (!w || !h) return;
      const dots = dotsRef.current;
      const mouse = mouseRef.current;
      const elapsed = (performance.now() - startTimeRef.current) / 1000;

      ctx.clearRect(0, 0, w, h);

      // Ambient center glow (elliptical via scale)
      ctx.save();
      ctx.translate(w * 0.5, h * 0.35);
      ctx.scale(1, 0.7);
      const glow = ctx.createRadialGradient(0, 0, 0, 0, 0, w * 0.45);
      glow.addColorStop(0, "rgba(99, 102, 241, 0.04)");
      glow.addColorStop(1, "rgba(99, 102, 241, 0)");
      ctx.fillStyle = glow;
      ctx.fillRect(-w * 0.5, -h * 0.5, w, h);
      ctx.restore();

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
        const baseAlpha = 0.055 + breath * 0.025;

        // Brighten edge if either connected dot is near cursor
        const edgeAlpha = Math.max(baseAlpha, Math.max(dotOpacities[si] ?? 0, dotOpacities[ti] ?? 0) * 0.3);

        ctx.beginPath();
        ctx.moveTo(s.x, s.y);
        ctx.lineTo(t.x, t.y);
        ctx.strokeStyle = `rgba(148, 163, 184, ${edgeAlpha})`;
        ctx.lineWidth = 0.5;
        ctx.stroke();
      }

      // Draw dots
      for (let i = 0; i < dots.length; i++) {
        const dot = dots[i]!;
        const alpha = dotOpacities[i] ?? 0;
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
