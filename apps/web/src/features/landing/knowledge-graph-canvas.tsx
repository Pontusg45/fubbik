import { useCallback, useEffect, useRef } from "react";

const NODE_LABELS = [
  "Convention",
  "Architecture",
  "API Endpoint",
  "Runbook",
  "Auth Flow",
  "Error Handling",
  "DB Schema",
  "Deployment",
  "Testing",
  "Logging",
  "API Auth",
  "Migrations",
  "CI Pipeline",
  "Code Style",
];

const NODE_TYPES: Array<
  "guide" | "document" | "reference" | "schema" | "checklist"
> = [
  "guide", // Convention
  "document", // Architecture
  "reference", // API Endpoint
  "checklist", // Runbook
  "guide", // Auth Flow
  "reference", // Error Handling
  "schema", // DB Schema
  "document", // Deployment
  "checklist", // Testing
  "reference", // Logging
  "reference", // API Auth
  "schema", // Migrations
  "document", // CI Pipeline
  "guide", // Code Style
];

const TYPE_COLORS: Record<string, [number, number, number]> = {
  guide: [99, 102, 241], // indigo
  document: [59, 130, 246], // blue
  reference: [20, 184, 166], // teal
  schema: [245, 158, 11], // amber
  checklist: [132, 204, 22], // lime
};

const EDGE_TEMPLATES: Array<[number, number, string]> = [
  [0, 5, "related_to"],
  [1, 6, "part_of"],
  [2, 10, "extends"],
  [3, 7, "depends_on"],
  [4, 5, "depends_on"],
  [6, 11, "part_of"],
  [8, 0, "references"],
  [9, 5, "supports"],
  [12, 7, "depends_on"],
  [13, 0, "extends"],
  [1, 4, "part_of"],
  [7, 12, "related_to"],
];

interface Node {
  x: number;
  y: number;
  vx: number;
  vy: number;
  w: number;
  h: number;
  label: string;
  color: [number, number, number];
}

function createNodes(
  canvas: HTMLCanvasElement,
  ctx: CanvasRenderingContext2D,
): Node[] {
  const w = canvas.width / (window.devicePixelRatio || 1);
  const h = canvas.height / (window.devicePixelRatio || 1);
  ctx.font = "10px system-ui, sans-serif";

  return NODE_LABELS.map((label, i) => {
    const textWidth = ctx.measureText(label).width;
    const nodeW = textWidth + 16;
    const nodeH = 24;
    return {
      x: Math.random() * (w - nodeW - 40) + 20,
      y: Math.random() * (h - nodeH - 40) + 20,
      vx: (Math.random() - 0.5) * 0.3,
      vy: (Math.random() - 0.5) * 0.3,
      w: nodeW,
      h: nodeH,
      label,
      color: TYPE_COLORS[NODE_TYPES[i]],
    };
  });
}

export function KnowledgeGraphCanvas() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const nodesRef = useRef<Node[]>([]);
  const mouseRef = useRef<{ x: number; y: number } | null>(null);
  const rafRef = useRef<number>(0);
  const reducedMotionRef = useRef(false);

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

    nodesRef.current = createNodes(canvas, ctx);
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
      const nodes = nodesRef.current;
      const mouse = mouseRef.current;

      ctx.clearRect(0, 0, w, h);

      // Mouse glow
      if (mouse) {
        const grad = ctx.createRadialGradient(
          mouse.x,
          mouse.y,
          0,
          mouse.x,
          mouse.y,
          200,
        );
        grad.addColorStop(0, "rgba(99, 102, 241, 0.08)");
        grad.addColorStop(1, "rgba(99, 102, 241, 0)");
        ctx.fillStyle = grad;
        ctx.fillRect(0, 0, w, h);
      }

      // Update positions (skip if reduced motion)
      if (!reducedMotionRef.current) {
        for (const node of nodes) {
          node.x += node.vx;
          node.y += node.vy;

          if (node.x <= 0) {
            node.x = 0;
            node.vx = Math.abs(node.vx);
          } else if (node.x + node.w >= w) {
            node.x = w - node.w;
            node.vx = -Math.abs(node.vx);
          }

          if (node.y <= 0) {
            node.y = 0;
            node.vy = Math.abs(node.vy);
          } else if (node.y + node.h >= h) {
            node.y = h - node.h;
            node.vy = -Math.abs(node.vy);
          }
        }
      }

      // Draw edges
      for (const [si, ti, relation] of EDGE_TEMPLATES) {
        const s = nodes[si];
        const t = nodes[ti];
        if (!s || !t) continue;

        const sx = s.x + s.w / 2;
        const sy = s.y + s.h / 2;
        const tx = t.x + t.w / 2;
        const ty = t.y + t.h / 2;
        const mx = (sx + tx) / 2;
        const my = (sy + ty) / 2;

        let mouseNear = false;
        if (mouse) {
          const dx = mouse.x - mx;
          const dy = mouse.y - my;
          mouseNear = Math.sqrt(dx * dx + dy * dy) < 200;
        }

        ctx.beginPath();
        ctx.moveTo(sx, sy);
        ctx.lineTo(tx, ty);

        if (mouseNear) {
          ctx.setLineDash([]);
          ctx.strokeStyle = "rgba(148, 163, 184, 0.5)";
          ctx.lineWidth = 1.2;
        } else {
          ctx.setLineDash([4, 4]);
          ctx.strokeStyle = "rgba(148, 163, 184, 0.15)";
          ctx.lineWidth = 1;
        }
        ctx.stroke();
        ctx.setLineDash([]);

        // Edge label
        if (mouseNear) {
          ctx.font = "8px system-ui, sans-serif";
          ctx.fillStyle = "rgba(148, 163, 184, 0.7)";
          ctx.textAlign = "center";
          ctx.textBaseline = "middle";
          ctx.fillText(relation, mx, my - 6);
        }
      }

      // Draw nodes
      ctx.font = "10px system-ui, sans-serif";
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";

      for (const node of nodes) {
        const cx = node.x + node.w / 2;
        const cy = node.y + node.h / 2;

        let alpha = 0.6;
        if (mouse) {
          const dx = mouse.x - cx;
          const dy = mouse.y - cy;
          const dist = Math.sqrt(dx * dx + dy * dy);
          if (dist < 200) {
            alpha = 0.6 + 0.4 * (1 - dist / 200);
          }
        }

        const [r, g, b] = node.color;

        // Fill
        ctx.beginPath();
        ctx.roundRect(node.x, node.y, node.w, node.h, 4);
        ctx.fillStyle = `rgba(${r}, ${g}, ${b}, ${alpha * 0.3})`;
        ctx.fill();

        // Stroke
        ctx.strokeStyle = `rgba(${r}, ${g}, ${b}, ${alpha})`;
        ctx.lineWidth = 1;
        ctx.stroke();

        // Text
        ctx.fillStyle = `rgba(${r}, ${g}, ${b}, ${alpha})`;
        ctx.fillText(node.label, cx, cy);
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
