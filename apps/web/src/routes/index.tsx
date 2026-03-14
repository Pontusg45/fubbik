import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { ArrowRight, Check, Copy, GitBranch, Layers, Map, Network, Scan, Sparkles, Terminal } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import FubbikLogo from "@/components/fubbik-logo";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { api } from "@/utils/api";

export const Route = createFileRoute("/")({
    component: LandingPage
});

/* ─── Animated constellation background ─── */

interface Star {
    x: number;
    y: number;
    vx: number;
    vy: number;
    r: number;
    pulse: number;
    pulseSpeed: number;
}

function ConstellationCanvas() {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const starsRef = useRef<Star[]>([]);
    const mouseRef = useRef({ x: -1000, y: -1000 });
    const frameRef = useRef(0);

    const init = useCallback((canvas: HTMLCanvasElement) => {
        const count = Math.floor((canvas.width * canvas.height) / 18000);
        starsRef.current = Array.from({ length: count }, () => ({
            x: Math.random() * canvas.width,
            y: Math.random() * canvas.height,
            vx: (Math.random() - 0.5) * 0.3,
            vy: (Math.random() - 0.5) * 0.3,
            r: Math.random() * 1.5 + 0.5,
            pulse: Math.random() * Math.PI * 2,
            pulseSpeed: Math.random() * 0.02 + 0.005
        }));
    }, []);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        const resize = () => {
            canvas.width = canvas.offsetWidth * window.devicePixelRatio;
            canvas.height = canvas.offsetHeight * window.devicePixelRatio;
            ctx.scale(window.devicePixelRatio, window.devicePixelRatio);
            init(canvas);
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

            const stars = starsRef.current;
            const mouse = mouseRef.current;

            for (const s of stars) {
                s.x += s.vx;
                s.y += s.vy;
                s.pulse += s.pulseSpeed;
                if (s.x < 0) s.x = w;
                if (s.x > w) s.x = 0;
                if (s.y < 0) s.y = h;
                if (s.y > h) s.y = 0;
            }

            // Draw connections
            const connectionDist = 120;
            for (let i = 0; i < stars.length; i++) {
                for (let j = i + 1; j < stars.length; j++) {
                    const dx = stars[i]!.x - stars[j]!.x;
                    const dy = stars[i]!.y - stars[j]!.y;
                    const d = Math.sqrt(dx * dx + dy * dy);
                    if (d < connectionDist) {
                        const alpha = (1 - d / connectionDist) * 0.15;
                        ctx.strokeStyle = `rgba(160, 180, 200, ${alpha})`;
                        ctx.lineWidth = 0.5;
                        ctx.beginPath();
                        ctx.moveTo(stars[i]!.x, stars[i]!.y);
                        ctx.lineTo(stars[j]!.x, stars[j]!.y);
                        ctx.stroke();
                    }
                }
            }

            // Draw stars
            for (const s of stars) {
                const dist = Math.sqrt((s.x - mouse.x) ** 2 + (s.y - mouse.y) ** 2);
                const glow = dist < 150 ? (1 - dist / 150) * 0.6 : 0;
                const pulseAlpha = 0.3 + Math.sin(s.pulse) * 0.15;
                const alpha = pulseAlpha + glow;
                ctx.fillStyle = `rgba(180, 200, 220, ${alpha})`;
                ctx.beginPath();
                ctx.arc(s.x, s.y, s.r + glow * 2, 0, Math.PI * 2);
                ctx.fill();
            }

            // Mouse glow
            if (mouse.x > 0) {
                const gradient = ctx.createRadialGradient(mouse.x, mouse.y, 0, mouse.x, mouse.y, 180);
                gradient.addColorStop(0, "rgba(140, 180, 255, 0.04)");
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

/* ─── Copy button ─── */

function CopyBtn({ text }: { text: string }) {
    const [ok, setOk] = useState(false);
    return (
        <button
            onClick={() => {
                navigator.clipboard.writeText(text);
                setOk(true);
                setTimeout(() => setOk(false), 1500);
            }}
            className="text-muted-foreground hover:text-foreground shrink-0 transition-colors"
        >
            {ok ? <Check className="size-3.5 text-emerald-400" /> : <Copy className="size-3.5" />}
        </button>
    );
}

/* ─── Feature pill ─── */

const features = [
    { icon: Layers, title: "Chunk-Based", desc: "Self-contained knowledge units with metadata, history, and typed relationships", docsSection: "chunks" },
    { icon: Network, title: "Knowledge Graphs", desc: "Visualize connections between chunks as an interactive force-directed graph", docsSection: "graph" },
    { icon: GitBranch, title: "Multi-Codebase", desc: "Organize knowledge per-project with auto-detection from git remotes", docsSection: "getting-started" },
    { icon: Sparkles, title: "AI-Native", desc: "MCP server, vocabulary parser, requirement generation, and semantic search", docsSection: "cli" },
    { icon: Scan, title: "Requirements", desc: "Given/When/Then specs with controlled vocabulary and multi-format export", docsSection: "requirements" },
    { icon: Map, title: "Health Dashboard", desc: "Detect orphans, stale content, and thin chunks across your knowledge base", docsSection: "chunks" }
];

function FeatureRow({ icon: Icon, title, desc, index, docsSection }: { icon: typeof Layers; title: string; desc: string; index: number; docsSection: string }) {
    return (
        <Link
            to="/docs"
            search={{ section: docsSection }}
            className="group border-border/50 hover:border-border hover:bg-muted/30 flex items-start gap-4 border-b py-5 transition-all duration-300 last:border-0"
            style={{ animationDelay: `${index * 80}ms` }}
        >
            <div className="bg-muted/50 group-hover:bg-muted mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-lg transition-colors">
                <Icon className="text-muted-foreground group-hover:text-foreground size-4 transition-colors" />
            </div>
            <div className="min-w-0">
                <div className="text-foreground mb-0.5 text-sm font-semibold tracking-tight">{title}</div>
                <div className="text-muted-foreground text-[13px] leading-relaxed">{desc}</div>
            </div>
            <ArrowRight className="text-muted-foreground/0 group-hover:text-muted-foreground mt-1 ml-auto size-4 shrink-0 transition-all duration-300 group-hover:translate-x-0.5" />
        </Link>
    );
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
                    <div className="text-foreground text-2xl font-bold tabular-nums tracking-tight">{s.value}</div>
                    <div className="text-muted-foreground text-[10px] font-medium uppercase tracking-widest">{s.label}</div>
                </div>
            ))}
        </div>
    );
}

/* ─── Main ─── */

function LandingPage() {
    return (
        <div className="relative min-h-[calc(100vh-4rem)] overflow-hidden">
            {/* Background */}
            <div className="pointer-events-none absolute inset-0 -top-16">
                <ConstellationCanvas />
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
                        Map your
                        <br />
                        <span className="bg-gradient-to-r from-blue-400 via-cyan-300 to-emerald-400 bg-clip-text text-transparent">
                            knowledge terrain
                        </span>
                    </h1>

                    <p className="text-muted-foreground mx-auto mb-8 max-w-lg text-base leading-relaxed">
                        A local-first knowledge framework for humans and machines. Store, navigate, and evolve structured knowledge as
                        interconnected chunks — each with its own metadata, history, and relationships.
                    </p>

                    <div className="mb-8 flex items-center justify-center gap-3">
                        <Button size="lg" render={<Link to="/dashboard" />}>
                            Open Dashboard
                            <ArrowRight className="size-4" />
                        </Button>
                        <Button variant="outline" size="lg" render={<Link to="/graph" />}>
                            <Network className="size-4" />
                            Explore Graph
                        </Button>
                        <Button variant="outline" size="lg" render={<Link to="/docs" search={{}} />}>
                            Docs
                        </Button>
                    </div>

                    {/* Install snippet */}
                    <div className="bg-muted/40 mx-auto flex max-w-sm items-center gap-3 rounded-lg border px-4 py-2.5 backdrop-blur-sm">
                        <Terminal className="text-muted-foreground size-4 shrink-0" />
                        <code className="text-muted-foreground flex-1 text-left font-mono text-sm">pnpm dev</code>
                        <CopyBtn text="pnpm dev" />
                    </div>

                </section>

                {/* Stats bar */}
                <section className="border-border/40 container mx-auto flex max-w-3xl items-center justify-center border-y px-4 py-8">
                    <LiveStats />
                </section>

                {/* Features */}
                <section className="container mx-auto max-w-2xl px-4 py-16">
                    <div className="mb-8 text-center">
                        <h2 className="text-foreground mb-2 text-2xl font-bold tracking-tight">Built for knowledge work</h2>
                        <p className="text-muted-foreground text-sm">Everything you need to capture, connect, and evolve what you know.</p>
                    </div>

                    <div className="border-border/50 rounded-xl border">
                        {features.map((f, i) => (
                            <FeatureRow key={f.title} {...f} index={i} />
                        ))}
                    </div>
                </section>

                {/* Capabilities grid */}
                <section className="container mx-auto max-w-3xl px-4 pb-16">
                    <div className="grid gap-3 sm:grid-cols-3">
                        {[
                            { label: "VS Code Extension", detail: "Browse and create chunks from your editor", section: "cli" },
                            { label: "MCP Server", detail: "AI agents query your knowledge base directly", section: "cli" },
                            { label: "CLI", detail: "fubbik context | fubbik generate claude.md", section: "cli" }
                        ].map(cap => (
                            <Link
                                key={cap.label}
                                to="/docs"
                                search={{ section: cap.section }}
                                className="bg-muted/20 hover:bg-muted/40 rounded-lg border p-4 transition-colors"
                            >
                                <div className="text-foreground mb-1 text-sm font-semibold">{cap.label}</div>
                                <div className="text-muted-foreground text-xs leading-relaxed">{cap.detail}</div>
                            </Link>
                        ))}
                    </div>
                </section>

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
                    </div>
                </footer>
            </div>
        </div>
    );
}
