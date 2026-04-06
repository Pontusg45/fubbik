# Landing Page & Features Page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the landing page with a developer-tool aesthetic and knowledge-graph background, and create a new `/features` page that explains Fubbik's mental model then groups features by concept.

**Architecture:** Two frontend routes (`/` and `/features`) sharing reusable components (knowledge graph canvas, terminal demo, install tabs, fade-in sections). The landing page is a complete rewrite of the existing `index.tsx`. The features page is a new route. No backend changes needed.

**Tech Stack:** TanStack Router, React, Tailwind CSS, Lucide icons, canvas API

---

### Task 1: Knowledge Graph Canvas Background

**Files:**
- Create: `apps/web/src/features/landing/knowledge-graph-canvas.tsx`

This replaces the existing `ConstellationCanvas` with a knowledge-graph-styled animated background. Nodes are small rounded rectangles with labels; edges show relation types.

- [ ] **Step 1: Create the knowledge graph canvas component**

```tsx
// apps/web/src/features/landing/knowledge-graph-canvas.tsx
import { useCallback, useEffect, useRef } from "react";

interface KnowledgeNode {
    x: number;
    y: number;
    vx: number;
    vy: number;
    label: string;
    type: string;
    w: number;
    h: number;
    pulse: number;
    pulseSpeed: number;
}

interface KnowledgeEdge {
    from: number;
    to: number;
    relation: string;
}

const NODE_LABELS = [
    { label: "Convention", type: "guide" },
    { label: "Architecture", type: "document" },
    { label: "API Endpoint", type: "reference" },
    { label: "Runbook", type: "checklist" },
    { label: "Auth Flow", type: "document" },
    { label: "Error Handling", type: "guide" },
    { label: "DB Schema", type: "schema" },
    { label: "Deployment", type: "checklist" },
    { label: "Testing", type: "guide" },
    { label: "Logging", type: "reference" },
    { label: "API Auth", type: "reference" },
    { label: "Migrations", type: "schema" },
    { label: "CI Pipeline", type: "checklist" },
    { label: "Code Style", type: "guide" },
];

const EDGE_TEMPLATES = [
    { from: 0, to: 5, relation: "related_to" },
    { from: 1, to: 6, relation: "part_of" },
    { from: 2, to: 10, relation: "extends" },
    { from: 3, to: 7, relation: "depends_on" },
    { from: 4, to: 5, relation: "depends_on" },
    { from: 6, to: 11, relation: "part_of" },
    { from: 8, to: 0, relation: "references" },
    { from: 9, to: 5, relation: "supports" },
    { from: 12, to: 7, relation: "depends_on" },
    { from: 13, to: 0, relation: "extends" },
    { from: 1, to: 4, relation: "part_of" },
    { from: 7, to: 12, relation: "related_to" },
];

const TYPE_COLORS: Record<string, string> = {
    guide: "99, 102, 241",      // indigo
    document: "59, 130, 246",   // blue
    reference: "20, 184, 166",  // teal
    schema: "245, 158, 11",     // amber
    checklist: "132, 204, 22",  // lime
};

function measureText(ctx: CanvasRenderingContext2D, text: string, fontSize: number): number {
    ctx.font = `${fontSize}px -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`;
    return ctx.measureText(text).width;
}

export function KnowledgeGraphCanvas() {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const nodesRef = useRef<KnowledgeNode[]>([]);
    const edgesRef = useRef<KnowledgeEdge[]>([]);
    const mouseRef = useRef({ x: -1000, y: -1000 });
    const frameRef = useRef(0);

    const init = useCallback((canvas: HTMLCanvasElement, ctx: CanvasRenderingContext2D) => {
        const w = canvas.offsetWidth;
        const h = canvas.offsetHeight;
        const fontSize = 10;

        nodesRef.current = NODE_LABELS.map(({ label, type }) => {
            const textW = measureText(ctx, label, fontSize);
            return {
                x: Math.random() * (w - 120) + 60,
                y: Math.random() * (h - 60) + 30,
                vx: (Math.random() - 0.5) * 0.15,
                vy: (Math.random() - 0.5) * 0.15,
                label,
                type,
                w: textW + 16,
                h: 24,
                pulse: Math.random() * Math.PI * 2,
                pulseSpeed: Math.random() * 0.01 + 0.003,
            };
        });

        edgesRef.current = EDGE_TEMPLATES.filter(
            e => e.from < nodesRef.current.length && e.to < nodesRef.current.length
        );
    }, []);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        // Check reduced motion preference
        const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

        const resize = () => {
            canvas.width = canvas.offsetWidth * window.devicePixelRatio;
            canvas.height = canvas.offsetHeight * window.devicePixelRatio;
            ctx.scale(window.devicePixelRatio, window.devicePixelRatio);
            init(canvas, ctx);
        };
        resize();
        window.addEventListener("resize", resize);

        const onMouse = (e: MouseEvent) => {
            const rect = canvas.getBoundingClientRect();
            mouseRef.current = { x: e.clientX - rect.left, y: e.clientY - rect.top };
        };
        canvas.addEventListener("mousemove", onMouse);

        let running = true;
        const draw = () => {
            if (!running) return;
            const w = canvas.offsetWidth;
            const h = canvas.offsetHeight;
            ctx.clearRect(0, 0, w, h);

            const nodes = nodesRef.current;
            const edges = edgesRef.current;
            const mouse = mouseRef.current;

            // Update positions (skip if reduced motion)
            if (!prefersReducedMotion) {
                for (const n of nodes) {
                    n.x += n.vx;
                    n.y += n.vy;
                    n.pulse += n.pulseSpeed;
                    if (n.x < 20) { n.x = 20; n.vx *= -1; }
                    if (n.x > w - 20) { n.x = w - 20; n.vx *= -1; }
                    if (n.y < 15) { n.y = 15; n.vy *= -1; }
                    if (n.y > h - 15) { n.y = h - 15; n.vy *= -1; }
                }
            }

            // Draw edges
            for (const edge of edges) {
                const a = nodes[edge.from]!;
                const b = nodes[edge.to]!;
                const dist = Math.sqrt((a.x - mouse.x) ** 2 + (a.y - mouse.y) ** 2);
                const nearMouse = dist < 200;
                const alpha = nearMouse ? 0.25 : 0.08;

                ctx.strokeStyle = `rgba(160, 180, 200, ${alpha})`;
                ctx.lineWidth = nearMouse ? 1 : 0.5;
                ctx.setLineDash(nearMouse ? [] : [4, 4]);
                ctx.beginPath();
                ctx.moveTo(a.x, a.y);
                ctx.lineTo(b.x, b.y);
                ctx.stroke();
                ctx.setLineDash([]);

                // Edge label (only when near mouse)
                if (nearMouse) {
                    const mx = (a.x + b.x) / 2;
                    const my = (a.y + b.y) / 2;
                    ctx.font = "8px -apple-system, BlinkMacSystemFont, sans-serif";
                    ctx.fillStyle = `rgba(160, 180, 200, 0.4)`;
                    ctx.textAlign = "center";
                    ctx.fillText(edge.relation, mx, my - 3);
                }
            }

            // Draw nodes
            for (const n of nodes) {
                const dist = Math.sqrt((n.x - mouse.x) ** 2 + (n.y - mouse.y) ** 2);
                const nearMouse = dist < 200;
                const pulseAlpha = 0.15 + Math.sin(n.pulse) * 0.05;
                const alpha = nearMouse ? 0.5 : pulseAlpha;
                const rgb = TYPE_COLORS[n.type] ?? "160, 180, 200";

                // Node background
                const rx = n.x - n.w / 2;
                const ry = n.y - n.h / 2;
                ctx.fillStyle = `rgba(${rgb}, ${alpha * 0.3})`;
                ctx.strokeStyle = `rgba(${rgb}, ${alpha})`;
                ctx.lineWidth = nearMouse ? 1 : 0.5;
                ctx.beginPath();
                ctx.roundRect(rx, ry, n.w, n.h, 4);
                ctx.fill();
                ctx.stroke();

                // Node label
                ctx.font = "10px -apple-system, BlinkMacSystemFont, sans-serif";
                ctx.fillStyle = `rgba(${rgb}, ${nearMouse ? 0.9 : alpha + 0.1})`;
                ctx.textAlign = "center";
                ctx.textBaseline = "middle";
                ctx.fillText(n.label, n.x, n.y);
            }

            // Mouse glow
            if (mouse.x > 0) {
                const gradient = ctx.createRadialGradient(mouse.x, mouse.y, 0, mouse.x, mouse.y, 200);
                gradient.addColorStop(0, "rgba(140, 180, 255, 0.03)");
                gradient.addColorStop(1, "rgba(140, 180, 255, 0)");
                ctx.fillStyle = gradient;
                ctx.fillRect(0, 0, w, h);
            }

            frameRef.current = requestAnimationFrame(draw);
        };
        draw();

        return () => {
            running = false;
            cancelAnimationFrame(frameRef.current);
            window.removeEventListener("resize", resize);
            canvas.removeEventListener("mousemove", onMouse);
        };
    }, [init]);

    return <canvas ref={canvasRef} className="pointer-events-auto absolute inset-0 size-full" />;
}
```

- [ ] **Step 2: Verify it renders**

Temporarily import and render in any page to visually confirm nodes with labels, edges, and mouse interaction work.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/features/landing/knowledge-graph-canvas.tsx
git commit -m "feat(landing): add knowledge graph canvas background component"
```

---

### Task 2: Terminal Demo Component

**Files:**
- Create: `apps/web/src/features/landing/terminal-demo.tsx`

Animated terminal showing a realistic Fubbik CLI workflow with typewriter effect.

- [ ] **Step 1: Create the terminal demo component**

```tsx
// apps/web/src/features/landing/terminal-demo.tsx
import { useEffect, useRef, useState } from "react";

interface TerminalLine {
    text: string;
    type: "command" | "output" | "success" | "muted";
    delay: number; // ms before this line appears
}

const DEMO_LINES: TerminalLine[] = [
    { text: '$ fubbik quick "Always use Effect for typed errors"', type: "command", delay: 0 },
    { text: "Created a8f3 — note", type: "success", delay: 800 },
    { text: "", type: "muted", delay: 400 },
    { text: '$ fubbik search "error handling"', type: "command", delay: 600 },
    { text: "3 chunks found across 2 codebases", type: "success", delay: 700 },
    { text: "", type: "muted", delay: 400 },
    { text: "$ fubbik context --for src/api/auth.ts", type: "command", delay: 600 },
    { text: "Found 5 relevant chunks (2,400 tokens)", type: "success", delay: 700 },
];

const LINE_COLORS: Record<string, string> = {
    command: "text-foreground",
    output: "text-muted-foreground",
    success: "text-emerald-400",
    muted: "text-muted-foreground/50",
};

export function TerminalDemo() {
    const [visibleLines, setVisibleLines] = useState(0);
    const [typing, setTyping] = useState("");
    const [typingIndex, setTypingIndex] = useState(0);
    const containerRef = useRef<HTMLDivElement>(null);
    const hasStarted = useRef(false);
    const prefersReducedMotion = useRef(false);

    useEffect(() => {
        prefersReducedMotion.current = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    }, []);

    // Start on scroll into view
    useEffect(() => {
        const el = containerRef.current;
        if (!el) return;
        const obs = new IntersectionObserver(
            ([entry]) => {
                if (entry?.isIntersecting && !hasStarted.current) {
                    hasStarted.current = true;
                    if (prefersReducedMotion.current) {
                        setVisibleLines(DEMO_LINES.length);
                    } else {
                        runSequence();
                    }
                    obs.disconnect();
                }
            },
            { threshold: 0.3 }
        );
        obs.observe(el);
        return () => obs.disconnect();
    }, []);

    function runSequence() {
        let totalDelay = 0;
        for (let i = 0; i < DEMO_LINES.length; i++) {
            totalDelay += DEMO_LINES[i]!.delay;
            const lineIndex = i;
            const line = DEMO_LINES[i]!;

            if (line.type === "command") {
                // Typewriter effect for commands
                const text = line.text;
                const charDelay = 25;
                for (let c = 0; c <= text.length; c++) {
                    const charIndex = c;
                    setTimeout(() => {
                        setTypingIndex(lineIndex);
                        setTyping(text.slice(0, charIndex));
                        if (charIndex === text.length) {
                            setVisibleLines(lineIndex + 1);
                            setTyping("");
                        }
                    }, totalDelay + c * charDelay);
                }
                totalDelay += text.length * charDelay;
            } else {
                setTimeout(() => {
                    setVisibleLines(lineIndex + 1);
                }, totalDelay);
            }
        }

        // Loop after pause
        setTimeout(() => {
            setVisibleLines(0);
            setTyping("");
            setTypingIndex(0);
            hasStarted.current = false;
            // Re-trigger
            const el = containerRef.current;
            if (el) {
                const obs = new IntersectionObserver(
                    ([entry]) => {
                        if (entry?.isIntersecting && !hasStarted.current) {
                            hasStarted.current = true;
                            runSequence();
                            obs.disconnect();
                        }
                    },
                    { threshold: 0.3 }
                );
                obs.observe(el);
            }
        }, totalDelay + 3000);
    }

    return (
        <div ref={containerRef} className="mx-auto max-w-lg overflow-hidden rounded-lg border bg-[#0d1117]">
            {/* Title bar */}
            <div className="flex items-center gap-1.5 border-b border-white/5 px-4 py-2.5">
                <div className="size-2.5 rounded-full bg-white/10" />
                <div className="size-2.5 rounded-full bg-white/10" />
                <div className="size-2.5 rounded-full bg-white/10" />
                <span className="ml-2 font-mono text-[10px] text-white/30">terminal</span>
            </div>
            {/* Content */}
            <div className="p-4 font-mono text-[13px] leading-relaxed">
                {DEMO_LINES.slice(0, visibleLines).map((line, i) => (
                    <div key={i} className={LINE_COLORS[line.type] ?? "text-muted-foreground"}>
                        {line.text || "\u00A0"}
                    </div>
                ))}
                {typing && (
                    <div className="text-foreground">
                        {typing}
                        <span className="animate-pulse">▊</span>
                    </div>
                )}
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/landing/terminal-demo.tsx
git commit -m "feat(landing): add animated terminal demo component"
```

---

### Task 3: Install Tabs Component

**Files:**
- Create: `apps/web/src/features/landing/install-tabs.tsx`

Tabbed component showing three install paths: Docker (coming soon), Local, npm (coming soon).

- [ ] **Step 1: Create the install tabs component**

```tsx
// apps/web/src/features/landing/install-tabs.tsx
import { Check, Copy } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";

function CopyButton({ text }: { text: string }) {
    const [copied, setCopied] = useState(false);
    return (
        <button
            onClick={() => {
                navigator.clipboard.writeText(text);
                setCopied(true);
                setTimeout(() => setCopied(false), 1500);
            }}
            className="absolute top-3 right-3 text-white/30 transition-colors hover:text-white/60"
        >
            {copied ? <Check className="size-3.5 text-emerald-400" /> : <Copy className="size-3.5" />}
        </button>
    );
}

const TABS = [
    {
        id: "local",
        label: "Local",
        badge: null,
        lines: [
            "git clone https://github.com/Pontusg45/fubbik.git",
            "cd fubbik",
            "pnpm install",
            "pnpm seed    # sample data",
            "pnpm dev     # localhost:3001",
        ],
    },
    {
        id: "docker",
        label: "Docker",
        badge: "soon",
        lines: [
            "git clone https://github.com/Pontusg45/fubbik.git",
            "cd fubbik",
            "docker compose up",
        ],
    },
    {
        id: "npm",
        label: "npm",
        badge: "soon",
        lines: [
            "npx create-fubbik my-knowledge-base",
            "cd my-knowledge-base",
            "pnpm dev",
        ],
    },
];

export function InstallTabs() {
    const [activeTab, setActiveTab] = useState("local");
    const tab = TABS.find(t => t.id === activeTab) ?? TABS[0]!;
    const copyText = tab.lines.join("\n");

    return (
        <div className="mx-auto max-w-lg">
            {/* Tab buttons */}
            <div className="flex gap-1 rounded-t-lg border border-b-0 bg-muted/30 p-1">
                {TABS.map(t => (
                    <button
                        key={t.id}
                        onClick={() => setActiveTab(t.id)}
                        className={`flex items-center gap-1.5 rounded-md px-3 py-1.5 font-mono text-xs transition-colors ${
                            activeTab === t.id
                                ? "bg-background text-foreground shadow-sm"
                                : "text-muted-foreground hover:text-foreground"
                        }`}
                    >
                        {t.label}
                        {t.badge && (
                            <Badge variant="outline" size="sm" className="text-[9px]">
                                {t.badge}
                            </Badge>
                        )}
                    </button>
                ))}
            </div>
            {/* Code block */}
            <div className="relative rounded-b-lg border bg-[#0d1117] p-4">
                <CopyButton text={copyText} />
                <pre className="font-mono text-[13px] leading-relaxed text-white/70">
                    {tab.lines.map((line, i) => (
                        <div key={i}>
                            {line.startsWith("#") || line.includes("#") ? (
                                <>
                                    {line.split("#")[0]}
                                    <span className="text-white/30">#{line.split("#").slice(1).join("#")}</span>
                                </>
                            ) : (
                                line
                            )}
                        </div>
                    ))}
                </pre>
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/web/src/features/landing/install-tabs.tsx
git commit -m "feat(landing): add install tabs component with copy support"
```

---

### Task 4: Landing Page Rewrite

**Files:**
- Modify: `apps/web/src/routes/index.tsx` (full rewrite)

Replace the entire landing page with the new developer-tool aesthetic design.

- [ ] **Step 1: Rewrite index.tsx**

Rewrite `apps/web/src/routes/index.tsx` keeping the reusable utilities (`useInView`, `FadeInSection`, `AnimatedNumber`, `LiveStats`) but replacing the structure with:

1. **Hero**: KnowledgeGraphCanvas background, bold tagline "Structured knowledge for your codebase", value prop line, two CTAs ("Get Started" scrolls to `#install`, "How it works" links to `/features`)
2. **Stats bar**: Keep the existing `LiveStats` component
3. **Terminal demo**: `TerminalDemo` component
4. **Integration grid**: "Works where you work" — 6 cards (CLI, Web UI, VS Code, MCP Server, API, Semantic Search) in 3x2 grid
5. **Feature teasers**: 4 horizontal cards linking to `/features#chunks`, `/features#connections`, `/features#context`, `/features#health`
6. **Get Started** (id="install"): `InstallTabs` component
7. **Footer**: Minimal — logo, tech stack, GitHub link

Key changes from current:
- Replace `ConstellationCanvas` with `KnowledgeGraphCanvas`
- Replace gradient text headline with clean bold text
- Replace "Open Dashboard" primary CTA with "Get Started" (scroll to install)
- Add "How it works" link to `/features`
- Replace feature list with compact teasers linking to `/features`
- Replace single `pnpm dev` snippet with `InstallTabs`
- Add `TerminalDemo` between stats and integration grid
- Keep `FadeInSection` wrappers for scroll animations

```tsx
// apps/web/src/routes/index.tsx
import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import {
    ArrowRight, Bot, Check, Code2, Copy, Github,
    Heart, Layers, LayoutDashboard, Network,
    Search, Sparkles, Terminal
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import FubbikLogo from "@/components/fubbik-logo";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { KnowledgeGraphCanvas } from "@/features/landing/knowledge-graph-canvas";
import { TerminalDemo } from "@/features/landing/terminal-demo";
import { InstallTabs } from "@/features/landing/install-tabs";
import { api } from "@/utils/api";

export const Route = createFileRoute("/")({
    component: LandingPage
});

/* ─── Scroll-triggered fade-up ─── */

function useInView(threshold = 0.15) {
    const ref = useRef<HTMLElement>(null);
    const [visible, setVisible] = useState(false);
    useEffect(() => {
        const el = ref.current;
        if (!el) return;
        const obs = new IntersectionObserver(
            ([entry]) => {
                if (entry?.isIntersecting) {
                    setVisible(true);
                    obs.disconnect();
                }
            },
            { threshold }
        );
        obs.observe(el);
        return () => obs.disconnect();
    }, [threshold]);
    return { ref, visible };
}

function FadeInSection({ children, className = "" }: { children: React.ReactNode; className?: string }) {
    const { ref, visible } = useInView(0.1);
    return (
        <div
            ref={ref as React.RefObject<HTMLDivElement>}
            className={`transition-all duration-700 ease-out ${visible ? "translate-y-0 opacity-100" : "translate-y-6 opacity-0"} ${className}`}
        >
            {children}
        </div>
    );
}

/* ─── Animated counter ─── */

function AnimatedNumber({ value, duration = 1200 }: { value: number; duration?: number }) {
    const [display, setDisplay] = useState(0);
    const { ref, visible } = useInView(0.3);

    useEffect(() => {
        if (!visible) return;
        const start = performance.now();
        const tick = (now: number) => {
            const elapsed = now - start;
            const progress = Math.min(elapsed / duration, 1);
            const eased = 1 - Math.pow(1 - progress, 3);
            setDisplay(Math.round(eased * value));
            if (progress < 1) requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
    }, [visible, value, duration]);

    return <span ref={ref as React.RefObject<HTMLSpanElement>}>{display}</span>;
}

/* ─── Stats ─── */

function LiveStats() {
    const { data } = useQuery({
        queryKey: ["landing-stats"],
        queryFn: async () => {
            const { data } = await api.api.stats.get();
            return data;
        }
    });
    if (!data) return null;
    const items = [
        { label: "Chunks", value: data.chunks },
        { label: "Connections", value: data.connections },
        { label: "Tags", value: data.tags }
    ];
    return (
        <div className="flex gap-6">
            {items.map(s => (
                <div key={s.label} className="text-center">
                    <div className="text-foreground text-2xl font-bold tabular-nums tracking-tight">
                        <AnimatedNumber value={s.value} />
                    </div>
                    <div className="text-muted-foreground text-[10px] font-medium uppercase tracking-widest">{s.label}</div>
                </div>
            ))}
        </div>
    );
}

/* ─── Integration cards data ─── */

const INTEGRATIONS = [
    { icon: Terminal, label: "CLI", desc: "Quick-add, search, context export, and plans from your terminal" },
    { icon: LayoutDashboard, label: "Web UI", desc: "Dashboard, graph visualization, chunk editor, and kanban" },
    { icon: Code2, label: "VS Code", desc: "File-aware chunk browsing and inline editing" },
    { icon: Bot, label: "MCP Server", desc: "AI agent integration with implementation tracking" },
    { icon: Network, label: "API", desc: "REST endpoints with Eden treaty type-safe client" },
    { icon: Search, label: "Semantic Search", desc: "Ollama-powered vector search across codebases" },
];

/* ─── Feature teasers data ─── */

const FEATURE_TEASERS = [
    { title: "Knowledge Graph", desc: "Visualize how your knowledge connects", anchor: "connections", icon: Network },
    { title: "Health Monitoring", desc: "Know when knowledge goes stale", anchor: "health", icon: Heart },
    { title: "AI-Native Context", desc: "Right knowledge at the right time", anchor: "context", icon: Sparkles },
    { title: "Capture & Create", desc: "From quick notes to structured decisions", anchor: "chunks", icon: Layers },
];

/* ─── Main ─── */

function LandingPage() {
    return (
        <div className="relative min-h-[calc(100vh-4rem)] overflow-hidden">
            {/* Background */}
            <div className="pointer-events-none absolute inset-0 -top-16">
                <KnowledgeGraphCanvas />
                <div className="from-background via-background/70 absolute right-0 bottom-0 left-0 h-[60%] bg-gradient-to-t to-transparent" />
            </div>

            <div className="relative z-10">
                {/* Hero */}
                <section className="container mx-auto max-w-3xl px-4 pt-20 pb-16 text-center">
                    <div className="mb-6 inline-flex items-center gap-3">
                        <FubbikLogo className="size-10 opacity-80" />
                        <span className="text-foreground text-2xl font-bold tracking-tight">fubbik</span>
                        <Badge variant="outline" className="border-border/60 font-mono text-[10px] uppercase tracking-widest">
                            v0.1
                        </Badge>
                    </div>

                    <h1 className="text-foreground mb-4 text-5xl leading-[1.1] font-bold tracking-tight sm:text-6xl">
                        Structured knowledge
                        <br />
                        for your codebase
                    </h1>

                    <p className="text-muted-foreground mx-auto mb-8 max-w-lg text-base leading-relaxed">
                        Store, connect, and evolve what your team knows — where your code lives.
                    </p>

                    <div className="flex items-center justify-center gap-3">
                        <Button size="lg" render={<a href="#install" />}>
                            Get Started
                            <ArrowRight className="size-4" />
                        </Button>
                        <Button variant="outline" size="lg" render={<Link to="/features" />}>
                            How it works
                        </Button>
                        <Button variant="outline" size="lg" render={<a href="https://github.com/Pontusg45/fubbik" target="_blank" rel="noopener noreferrer" />}>
                            <Github className="size-4" />
                            GitHub
                        </Button>
                    </div>
                </section>

                {/* Stats bar */}
                <section className="border-border/40 container mx-auto flex max-w-3xl items-center justify-center border-y px-4 py-8">
                    <LiveStats />
                </section>

                {/* Terminal demo */}
                <FadeInSection>
                    <section className="container mx-auto max-w-3xl px-4 py-16">
                        <TerminalDemo />
                    </section>
                </FadeInSection>

                {/* Integration grid */}
                <FadeInSection>
                    <section className="container mx-auto max-w-3xl px-4 pb-16">
                        <div className="mb-8 text-center">
                            <h2 className="text-foreground mb-2 text-2xl font-bold tracking-tight">Works where you work</h2>
                            <p className="text-muted-foreground text-sm">One knowledge base, every surface.</p>
                        </div>

                        <div className="grid gap-3 sm:grid-cols-3">
                            {INTEGRATIONS.map(cap => (
                                <div
                                    key={cap.label}
                                    className="bg-muted/20 hover:bg-muted/40 rounded-lg border p-4 transition-colors"
                                >
                                    <div className="mb-2 flex items-center gap-2">
                                        <cap.icon className="text-muted-foreground size-4" />
                                        <span className="text-foreground text-sm font-semibold">{cap.label}</span>
                                    </div>
                                    <div className="text-muted-foreground text-xs leading-relaxed">{cap.desc}</div>
                                </div>
                            ))}
                        </div>
                    </section>
                </FadeInSection>

                {/* Feature teasers */}
                <FadeInSection>
                    <section className="container mx-auto max-w-2xl px-4 pb-16">
                        <div className="border-border/50 divide-border/50 divide-y rounded-xl border">
                            {FEATURE_TEASERS.map(f => (
                                <Link
                                    key={f.title}
                                    to="/features"
                                    hash={f.anchor}
                                    className="group flex items-center gap-4 px-5 py-4 transition-colors hover:bg-muted/30"
                                >
                                    <div className="bg-muted/50 group-hover:bg-muted flex size-9 shrink-0 items-center justify-center rounded-lg transition-colors">
                                        <f.icon className="text-muted-foreground group-hover:text-foreground size-4 transition-colors" />
                                    </div>
                                    <div className="min-w-0 flex-1">
                                        <div className="text-foreground text-sm font-semibold">{f.title}</div>
                                        <div className="text-muted-foreground text-xs">{f.desc}</div>
                                    </div>
                                    <ArrowRight className="text-muted-foreground/0 group-hover:text-muted-foreground size-4 shrink-0 transition-all duration-300 group-hover:translate-x-0.5" />
                                </Link>
                            ))}
                        </div>
                    </section>
                </FadeInSection>

                {/* Get Started */}
                <FadeInSection>
                    <section id="install" className="container mx-auto max-w-3xl px-4 pb-16">
                        <div className="mb-8 text-center">
                            <h2 className="text-foreground mb-2 text-2xl font-bold tracking-tight">Get started</h2>
                            <p className="text-muted-foreground text-sm">Up and running in under a minute.</p>
                        </div>
                        <InstallTabs />
                    </section>
                </FadeInSection>

                {/* Footer */}
                <footer className="border-border/40 border-t">
                    <div className="container mx-auto flex max-w-3xl items-center justify-between px-4 py-6">
                        <div className="text-muted-foreground flex items-center gap-2 text-xs">
                            <FubbikLogo className="size-3.5 opacity-50" />
                            <span className="opacity-50">fubbik</span>
                        </div>
                        <div className="text-muted-foreground flex gap-4 font-mono text-[10px] uppercase tracking-widest">
                            <span>TanStack</span>
                            <span>Elysia</span>
                            <span>Drizzle</span>
                            <span>Effect</span>
                        </div>
                        <a href="https://github.com/Pontusg45/fubbik" target="_blank" rel="noopener noreferrer" className="text-muted-foreground hover:text-foreground transition-colors">
                            <Github className="size-4" />
                        </a>
                    </div>
                </footer>
            </div>
        </div>
    );
}
```

- [ ] **Step 2: Verify visually**

Run: `pnpm dev`, navigate to `/`.
Expected: Knowledge graph background, clean hero, terminal demo, integration grid, feature teasers, install tabs, footer.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/index.tsx
git commit -m "feat(landing): redesign with developer-tool aesthetic and knowledge graph background"
```

---

### Task 5: Features Page — Header and Model Diagram

**Files:**
- Create: `apps/web/src/routes/features.tsx`

Create the features page with the header and mental model flow diagram. Subsequent tasks will add the concept sections.

- [ ] **Step 1: Create features.tsx with header and model**

```tsx
// apps/web/src/routes/features.tsx
import { createFileRoute, Link } from "@tanstack/react-router";
import {
    ArrowRight, Bot, Code2, GitFork, Heart, Layers, LayoutDashboard,
    Link2, Network, Search, Sparkles, Terminal, Zap
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import FubbikLogo from "@/components/fubbik-logo";
import { Badge } from "@/components/ui/badge";
import { InstallTabs } from "@/features/landing/install-tabs";

export const Route = createFileRoute("/features")({
    component: FeaturesPage
});

/* ─── Scroll-triggered fade-up ─── */

function useInView(threshold = 0.15) {
    const ref = useRef<HTMLElement>(null);
    const [visible, setVisible] = useState(false);
    useEffect(() => {
        const el = ref.current;
        if (!el) return;
        const obs = new IntersectionObserver(
            ([entry]) => {
                if (entry?.isIntersecting) {
                    setVisible(true);
                    obs.disconnect();
                }
            },
            { threshold }
        );
        obs.observe(el);
        return () => obs.disconnect();
    }, [threshold]);
    return { ref, visible };
}

function FadeInSection({ children, className = "" }: { children: React.ReactNode; className?: string }) {
    const { ref, visible } = useInView(0.1);
    return (
        <div
            ref={ref as React.RefObject<HTMLDivElement>}
            className={`transition-all duration-700 ease-out ${visible ? "translate-y-0 opacity-100" : "translate-y-6 opacity-0"} ${className}`}
        >
            {children}
        </div>
    );
}

/* ─── Model step card ─── */

function ModelStep({ icon: Icon, title, desc }: { icon: typeof Layers; title: string; desc: string }) {
    return (
        <div className="flex flex-1 flex-col items-center rounded-xl border bg-muted/20 p-6 text-center">
            <div className="mb-3 flex size-12 items-center justify-center rounded-xl bg-muted/50">
                <Icon className="text-muted-foreground size-5" />
            </div>
            <h3 className="text-foreground mb-1 text-sm font-semibold">{title}</h3>
            <p className="text-muted-foreground text-xs leading-relaxed">{desc}</p>
        </div>
    );
}

function ModelArrow() {
    return (
        <div className="hidden items-center text-muted-foreground/30 md:flex">
            <ArrowRight className="size-5" />
        </div>
    );
}

/* ─── Main ─── */

function FeaturesPage() {
    return (
        <div className="min-h-screen">
            {/* Header */}
            <section className="container mx-auto max-w-4xl px-4 pt-16 pb-12 text-center">
                <h1 className="text-foreground mb-3 text-4xl font-bold tracking-tight sm:text-5xl">How Fubbik Works</h1>
                <p className="text-muted-foreground text-base">The mental model, then the tools.</p>
            </section>

            {/* Model diagram */}
            <FadeInSection>
                <section id="model" className="container mx-auto max-w-4xl px-4 pb-16">
                    <div className="flex flex-col items-stretch gap-3 md:flex-row md:items-center">
                        <ModelStep icon={Layers} title="Chunks" desc="Atomic units of knowledge" />
                        <ModelArrow />
                        <ModelStep icon={Link2} title="Connections" desc="Typed edges between them" />
                        <ModelArrow />
                        <ModelStep icon={Sparkles} title="Context" desc="Smart retrieval" />
                        <ModelArrow />
                        <ModelStep icon={Zap} title="Action" desc="Where you use it" />
                    </div>
                </section>
            </FadeInSection>

            {/* === Concept Sections (Tasks 6-9 add content here) === */}

            {/* Chunks section */}
            <FadeInSection>
                <section id="chunks" className="border-border/40 border-t">
                    <div className="container mx-auto max-w-3xl px-4 py-16">
                        <div className="mb-2 text-xs font-semibold uppercase tracking-widest text-muted-foreground">The Model</div>
                        <h2 className="text-foreground mb-3 text-2xl font-bold tracking-tight">Atomic units of knowledge</h2>
                        <p className="text-muted-foreground mb-8 max-w-xl text-sm leading-relaxed">
                            Each chunk is a self-contained piece of knowledge — a convention, architecture decision, runbook, or API reference.
                            Unlike monolithic docs, chunks are typed, tagged, scoped to codebases, and linked to the files they describe.
                        </p>

                        {/* Example chunk card */}
                        <div className="mb-8 rounded-xl border bg-muted/10 p-5">
                            <div className="mb-3 flex items-center gap-2">
                                <Badge variant="outline" size="sm">Convention</Badge>
                                <span className="text-muted-foreground text-xs">packages/api/**</span>
                            </div>
                            <h3 className="text-foreground mb-1 text-base font-semibold">Always use Effect for typed errors</h3>
                            <div className="mb-3 flex gap-1.5">
                                <Badge variant="secondary" size="sm">#backend</Badge>
                                <Badge variant="secondary" size="sm">#error-handling</Badge>
                            </div>
                            <p className="text-muted-foreground mb-3 text-sm leading-relaxed">
                                Use Effect.tryPromise with tagged error types (DatabaseError, NotFoundError, AuthError).
                                The global error handler extracts the _tag and maps to HTTP status codes.
                            </p>
                            <div className="flex items-center gap-4 text-xs text-muted-foreground">
                                <span>Health: <span className="text-emerald-400 font-semibold">82/100</span></span>
                                <span>3 connections</span>
                                <span>Updated 5 days ago</span>
                            </div>
                        </div>

                        {/* Feature list */}
                        <div className="grid gap-x-8 gap-y-2 text-sm sm:grid-cols-2">
                            {[
                                "5 types: note, document, reference, schema, checklist",
                                "Tags with typed categories",
                                "Scope metadata and appliesTo glob patterns",
                                "File references linking chunks to code",
                                "Decision context: rationale, alternatives, consequences",
                                "Version history (append-only)",
                                "Templates (built-in + custom)",
                                "AI enrichment: summary, aliases, notAbout",
                            ].map(item => (
                                <div key={item} className="text-muted-foreground flex items-start gap-2 py-1">
                                    <span className="mt-1.5 size-1 shrink-0 rounded-full bg-muted-foreground/50" />
                                    {item}
                                </div>
                            ))}
                        </div>
                    </div>
                </section>
            </FadeInSection>

            {/* Connections section */}
            <FadeInSection>
                <section id="connections" className="border-border/40 border-t">
                    <div className="container mx-auto max-w-3xl px-4 py-16">
                        <div className="mb-2 text-xs font-semibold uppercase tracking-widest text-muted-foreground">The Model</div>
                        <h2 className="text-foreground mb-3 text-2xl font-bold tracking-tight">Typed relationships between knowledge</h2>
                        <p className="text-muted-foreground mb-8 max-w-xl text-sm leading-relaxed">
                            Connections are directed edges with semantic meaning. They're global — not codebase-scoped — enabling
                            cross-project knowledge linking. A convention in your backend can reference a schema in your infrastructure repo.
                        </p>

                        {/* Mini graph example */}
                        <div className="mb-8 rounded-xl border bg-muted/10 p-6">
                            <div className="space-y-3 text-sm">
                                {[
                                    { from: "Auth Middleware", rel: "depends_on", to: "Session Token Format" },
                                    { from: "Auth Middleware", rel: "part_of", to: "Authentication System" },
                                    { from: "JWT Tokens", rel: "contradicts", to: "Session Cookies" },
                                    { from: "OAuth2 Flow", rel: "extends", to: "Auth Middleware" },
                                ].map((edge, i) => (
                                    <div key={i} className="flex items-center gap-2">
                                        <span className="text-foreground font-medium min-w-0 truncate">{edge.from}</span>
                                        <Badge variant="outline" size="sm" className="shrink-0 font-mono text-[10px]">{edge.rel}</Badge>
                                        <ArrowRight className="size-3 shrink-0 text-muted-foreground" />
                                        <span className="text-foreground font-medium min-w-0 truncate">{edge.to}</span>
                                    </div>
                                ))}
                            </div>
                        </div>

                        {/* Relation types */}
                        <div className="mb-6 flex flex-wrap gap-2">
                            {["depends_on", "part_of", "extends", "references", "supports", "contradicts", "alternative_to", "related_to"].map(r => (
                                <Badge key={r} variant="outline" size="sm" className="font-mono text-[10px]">{r}</Badge>
                            ))}
                        </div>

                        {/* Feature list */}
                        <div className="grid gap-x-8 gap-y-2 text-sm sm:grid-cols-2">
                            {[
                                "Dependency tree (incoming/outgoing by type)",
                                "Related chunk suggestions via embeddings",
                                "Connection creation with relation picker",
                                "Cross-codebase connections",
                                "Graph visualization with focus mode",
                                "Saveable filter presets",
                            ].map(item => (
                                <div key={item} className="text-muted-foreground flex items-start gap-2 py-1">
                                    <span className="mt-1.5 size-1 shrink-0 rounded-full bg-muted-foreground/50" />
                                    {item}
                                </div>
                            ))}
                        </div>
                    </div>
                </section>
            </FadeInSection>

            {/* Context section */}
            <FadeInSection>
                <section id="context" className="border-border/40 border-t">
                    <div className="container mx-auto max-w-3xl px-4 py-16">
                        <div className="mb-2 text-xs font-semibold uppercase tracking-widest text-muted-foreground">The Model</div>
                        <h2 className="text-foreground mb-3 text-2xl font-bold tracking-tight">The right knowledge at the right time</h2>
                        <p className="text-muted-foreground mb-8 max-w-xl text-sm leading-relaxed">
                            Fubbik doesn't just store knowledge — it delivers it where you need it. File-aware context matching,
                            token-budgeted exports, and AI agent integration ensure the right chunks reach the right tool.
                        </p>

                        {/* Context flow */}
                        <div className="mb-8 rounded-xl border bg-muted/10 p-5">
                            <div className="space-y-4">
                                <div className="flex items-start gap-3">
                                    <div className="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded bg-muted/50 text-xs font-bold text-muted-foreground">1</div>
                                    <div>
                                        <div className="text-foreground text-sm font-medium">File path</div>
                                        <code className="text-muted-foreground text-xs">src/api/auth.ts</code>
                                    </div>
                                </div>
                                <div className="ml-3 h-4 border-l border-dashed border-border/50" />
                                <div className="flex items-start gap-3">
                                    <div className="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded bg-muted/50 text-xs font-bold text-muted-foreground">2</div>
                                    <div>
                                        <div className="text-foreground text-sm font-medium">Match</div>
                                        <div className="text-muted-foreground text-xs">File refs + glob patterns + dependency analysis → 5 relevant chunks</div>
                                    </div>
                                </div>
                                <div className="ml-3 h-4 border-l border-dashed border-border/50" />
                                <div className="flex items-start gap-3">
                                    <div className="mt-0.5 flex size-6 shrink-0 items-center justify-center rounded bg-muted/50 text-xs font-bold text-muted-foreground">3</div>
                                    <div>
                                        <div className="text-foreground text-sm font-medium">Deliver</div>
                                        <div className="text-muted-foreground text-xs">Token-budgeted export → CLAUDE.md, MCP response, or CLI output</div>
                                    </div>
                                </div>
                            </div>
                        </div>

                        {/* Feature list */}
                        <div className="grid gap-x-8 gap-y-2 text-sm sm:grid-cols-2">
                            {[
                                "File-aware context: refs, globs, dependencies",
                                "Semantic search via Ollama embeddings",
                                "Federated cross-codebase search",
                                "Token-budgeted export with relevance scoring",
                                "CLAUDE.md generation from tagged chunks",
                                "MCP server with 15+ AI agent tools",
                                "Context-for-file API endpoint",
                            ].map(item => (
                                <div key={item} className="text-muted-foreground flex items-start gap-2 py-1">
                                    <span className="mt-1.5 size-1 shrink-0 rounded-full bg-muted-foreground/50" />
                                    {item}
                                </div>
                            ))}
                        </div>
                    </div>
                </section>
            </FadeInSection>

            {/* Health section */}
            <FadeInSection>
                <section id="health" className="border-border/40 border-t">
                    <div className="container mx-auto max-w-3xl px-4 py-16">
                        <div className="mb-2 text-xs font-semibold uppercase tracking-widest text-muted-foreground">The Model</div>
                        <h2 className="text-foreground mb-3 text-2xl font-bold tracking-tight">Knowledge that maintains itself</h2>
                        <p className="text-muted-foreground mb-8 max-w-xl text-sm leading-relaxed">
                            Knowledge rots silently. Fubbik detects staleness, flags duplicates, scores health, and surfaces what
                            needs attention — before you discover outdated docs in production.
                        </p>

                        {/* Health score breakdown */}
                        <div className="mb-8 rounded-xl border bg-muted/10 p-5">
                            <div className="mb-4 text-sm font-semibold text-foreground">Health Score Breakdown</div>
                            <div className="space-y-3">
                                {[
                                    { label: "Freshness", score: 20, max: 25, desc: "Updated 12 days ago" },
                                    { label: "Completeness", score: 25, max: 25, desc: "Has rationale, alternatives, consequences" },
                                    { label: "Richness", score: 18, max: 25, desc: "Has summary, missing embedding" },
                                    { label: "Connectivity", score: 25, max: 25, desc: "4 connections" },
                                ].map(item => (
                                    <div key={item.label}>
                                        <div className="mb-1 flex items-center justify-between text-xs">
                                            <span className="text-foreground font-medium">{item.label}</span>
                                            <span className="text-muted-foreground">{item.score}/{item.max}</span>
                                        </div>
                                        <div className="h-1.5 rounded-full bg-muted/50">
                                            <div
                                                className="h-full rounded-full bg-emerald-500/60"
                                                style={{ width: `${(item.score / item.max) * 100}%` }}
                                            />
                                        </div>
                                        <div className="mt-0.5 text-[10px] text-muted-foreground">{item.desc}</div>
                                    </div>
                                ))}
                            </div>
                            <div className="mt-4 flex items-center justify-between border-t border-border/30 pt-3">
                                <span className="text-sm font-semibold text-foreground">Total</span>
                                <span className="text-lg font-bold text-emerald-400">88/100</span>
                            </div>
                        </div>

                        {/* Staleness banner example */}
                        <div className="mb-8 rounded-lg border border-amber-500/30 bg-amber-500/5 p-3">
                            <div className="flex items-start gap-2 text-sm">
                                <Heart className="mt-0.5 size-4 shrink-0 text-amber-500" />
                                <span className="text-foreground">Files linked to this chunk changed 3 days ago: <code className="text-muted-foreground">src/auth/middleware.ts</code></span>
                            </div>
                        </div>

                        {/* Feature list */}
                        <div className="grid gap-x-8 gap-y-2 text-sm sm:grid-cols-2">
                            {[
                                "Health scores: freshness + completeness + richness + connectivity",
                                "Staleness detection: age, file changes, duplicate divergence",
                                "Dashboard Attention Needed widget",
                                "Nav badge showing stale chunk count",
                                "Chunk detail banners with dismiss/suppress",
                                "Knowledge health page: orphans, stale, thin chunks",
                            ].map(item => (
                                <div key={item} className="text-muted-foreground flex items-start gap-2 py-1">
                                    <span className="mt-1.5 size-1 shrink-0 rounded-full bg-muted-foreground/50" />
                                    {item}
                                </div>
                            ))}
                        </div>
                    </div>
                </section>
            </FadeInSection>

            {/* Surfaces */}
            <FadeInSection>
                <section id="surfaces" className="border-border/40 border-t">
                    <div className="container mx-auto max-w-4xl px-4 py-16">
                        <div className="mb-8 text-center">
                            <h2 className="text-foreground mb-2 text-2xl font-bold tracking-tight">Where you interact</h2>
                            <p className="text-muted-foreground text-sm">Five surfaces, one knowledge base.</p>
                        </div>

                        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                            {[
                                {
                                    icon: LayoutDashboard, title: "Web UI",
                                    items: ["Dashboard with stats, favorites, and attention widget", "Knowledge graph with focus mode and filter presets", "Chunk editor with templates, autosave, and duplicate detection", "Requirements with BDD steps and interactive plan checklists"]
                                },
                                {
                                    icon: Terminal, title: "CLI",
                                    items: ["fubbik quick for instant capture", "fubbik search / fubbik context for retrieval", "fubbik plan for implementation tracking", "fubbik sync-claude-md for AI context generation"]
                                },
                                {
                                    icon: Code2, title: "VS Code",
                                    items: ["Sidebar with type/tag/sort filtering", "File-aware chunk surfacing in active editor", "Inline editing, quick-add, status bar"]
                                },
                                {
                                    icon: Bot, title: "MCP Server",
                                    items: ["15+ tools for AI agents", "Implementation sessions with review briefs", "Plan creation and step tracking", "Context retrieval and CLAUDE.md sync"]
                                },
                                {
                                    icon: Network, title: "API",
                                    items: ["REST endpoints with Swagger/OpenAPI docs", "Eden treaty for type-safe client", "Effect-based error handling with tagged types"]
                                },
                            ].map(surface => (
                                <div key={surface.title} className="rounded-lg border bg-muted/10 p-5">
                                    <div className="mb-3 flex items-center gap-2">
                                        <surface.icon className="text-muted-foreground size-4" />
                                        <span className="text-foreground text-sm font-semibold">{surface.title}</span>
                                    </div>
                                    <ul className="space-y-1.5">
                                        {surface.items.map(item => (
                                            <li key={item} className="text-muted-foreground flex items-start gap-2 text-xs leading-relaxed">
                                                <span className="mt-1.5 size-1 shrink-0 rounded-full bg-muted-foreground/40" />
                                                {item}
                                            </li>
                                        ))}
                                    </ul>
                                </div>
                            ))}
                        </div>
                    </div>
                </section>
            </FadeInSection>

            {/* Bottom CTA */}
            <FadeInSection>
                <section className="border-border/40 border-t">
                    <div className="container mx-auto max-w-3xl px-4 py-16">
                        <div className="mb-8 text-center">
                            <h2 className="text-foreground mb-2 text-2xl font-bold tracking-tight">Ready to try it?</h2>
                            <p className="text-muted-foreground text-sm">Up and running in under a minute.</p>
                        </div>
                        <InstallTabs />
                    </div>
                </section>
            </FadeInSection>
        </div>
    );
}
```

- [ ] **Step 2: Verify visually**

Run: `pnpm dev`, navigate to `/features`.
Expected: Header, model diagram, 4 concept sections with examples and feature lists, surfaces grid, install CTA.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/features.tsx
git commit -m "feat: add /features page with mental model, concept sections, and surfaces grid"
```

---

### Task 6: Navigation Link and Final Polish

**Files:**
- Modify: `apps/web/src/routes/__root.tsx` (add Features nav link)

- [ ] **Step 1: Add Features link to nav**

In `apps/web/src/routes/__root.tsx`, find the nav links section and add a "Features" link. Look for where other links like "Dashboard", "Chunks", "Graph" are defined. Add after the existing links or in the appropriate position:

```tsx
<Link to="/features" className="...same classes as other nav links...">Features</Link>
```

Also add it to the mobile nav sheet if one exists.

- [ ] **Step 2: Verify navigation**

Click "Features" in the nav bar and the "How it works" button on the landing page. Both should navigate to `/features`.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/__root.tsx
git commit -m "feat(nav): add Features link to navigation"
```

---

### Task 7: Smoke Test

- [ ] **Step 1: Run type checks**

Run: `pnpm run check-types --filter=web`
Expected: No type errors.

- [ ] **Step 2: Visual smoke test**

1. Landing page (`/`): Knowledge graph background animates, hero text is clean, terminal demo auto-plays, integration grid renders 6 cards, feature teasers link to `/features#section`, install tabs switch between Local/Docker/npm, footer shows
2. Features page (`/features`): Model diagram shows 4 steps with arrows, each concept section has example + feature list, surfaces grid shows 5 cards, bottom CTA has install tabs
3. Mobile: Both pages are responsive, terminal demo scales, grids collapse
4. Hash navigation: `/features#chunks`, `/features#connections`, `/features#context`, `/features#health` all scroll to correct sections

- [ ] **Step 3: Final commit if any fixes needed**
