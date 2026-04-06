import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { ArrowRight, Bot, Code2, Github, Heart, Layers, LayoutDashboard, Network, Search, Sparkles, Terminal } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import FubbikLogo from "@/components/fubbik-logo";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { InstallTabs } from "@/features/landing/install-tabs";
import { KnowledgeGraphCanvas } from "@/features/landing/knowledge-graph-canvas";
import { TerminalDemo } from "@/features/landing/terminal-demo";
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
            // ease-out cubic
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

/* ─── Integration grid data ─── */

const integrations = [
    { icon: Terminal, label: "CLI", desc: "Full-featured command line for chunk management, context export, and docs import." },
    { icon: LayoutDashboard, label: "Web UI", desc: "Dashboard, graph visualization, health monitoring, and knowledge management." },
    { icon: Code2, label: "VS Code", desc: "Browse, search, and create chunks without leaving your editor." },
    { icon: Bot, label: "MCP Server", desc: "AI agents query your knowledge base directly via Model Context Protocol." },
    { icon: Network, label: "API", desc: "RESTful API with OpenAPI docs for custom integrations and automation." },
    { icon: Search, label: "Semantic Search", desc: "Find knowledge by meaning with local embeddings via Ollama." }
];

/* ─── Feature teasers data ─── */

const featureTeasers = [
    { icon: Layers, hash: "#chunks", title: "Knowledge Graph", desc: "Typed chunks with metadata, history, and directed relationships." },
    { icon: Heart, hash: "#health", title: "Health Monitoring", desc: "Freshness, completeness, richness, and connectivity scores." },
    { icon: Sparkles, hash: "#context", title: "AI-Native Context", desc: "Token-budgeted exports and CLAUDE.md generation for AI agents." },
    { icon: Terminal, hash: "#connections", title: "Capture & Create", desc: "CLI, web UI, VS Code, and MCP -- create knowledge from anywhere." }
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

                    <div className="mb-8 flex flex-col items-center gap-3 sm:flex-row sm:justify-center">
                        <Button size="lg" className="w-full sm:w-auto" render={<a href="#install" />}>
                            Get Started
                            <ArrowRight className="size-4" />
                        </Button>
                        <Button variant="outline" size="lg" className="w-full sm:w-auto" render={<a href="/features" />}>
                            How it works
                        </Button>
                        <Button variant="outline" size="lg" className="w-full sm:w-auto" render={<a href="https://github.com/Pontusg45/fubbik" target="_blank" rel="noopener noreferrer" />}>
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
                            <p className="text-muted-foreground text-sm">Integrate with your existing tools and workflows.</p>
                        </div>

                        <div className="grid gap-3 sm:grid-cols-3">
                            {integrations.map(item => (
                                <div
                                    key={item.label}
                                    className="bg-muted/20 hover:bg-muted/40 rounded-lg border p-4 transition-colors"
                                >
                                    <div className="mb-2 flex items-center gap-2">
                                        <item.icon className="text-muted-foreground size-4" />
                                        <span className="text-foreground text-sm font-semibold">{item.label}</span>
                                    </div>
                                    <div className="text-muted-foreground text-xs leading-relaxed">{item.desc}</div>
                                </div>
                            ))}
                        </div>
                    </section>
                </FadeInSection>

                {/* Feature teasers */}
                <FadeInSection>
                    <section className="container mx-auto max-w-2xl px-4 pb-16">
                        <div className="border-border/50 divide-border/50 rounded-xl border divide-y">
                            {featureTeasers.map((item, i) => (
                                <a
                                    key={item.title}
                                    href={`/features${item.hash}`}
                                    className="group flex items-start gap-4 px-5 py-5 transition-all duration-300 hover:bg-muted/30"
                                    style={{ animationDelay: `${i * 80}ms` }}
                                >
                                    <div className="bg-muted/50 group-hover:bg-muted mt-0.5 flex size-9 shrink-0 items-center justify-center rounded-lg transition-colors">
                                        <item.icon className="text-muted-foreground group-hover:text-foreground size-4 transition-colors" />
                                    </div>
                                    <div className="min-w-0">
                                        <div className="text-foreground mb-0.5 text-sm font-semibold tracking-tight">{item.title}</div>
                                        <div className="text-muted-foreground text-[13px] leading-relaxed">{item.desc}</div>
                                    </div>
                                    <ArrowRight className="text-muted-foreground/0 group-hover:text-muted-foreground mt-1 ml-auto size-4 shrink-0 transition-all duration-300 group-hover:translate-x-0.5" />
                                </a>
                            ))}
                        </div>
                    </section>
                </FadeInSection>

                {/* Get Started */}
                <FadeInSection>
                    <section id="install" className="container mx-auto max-w-3xl px-4 pb-16">
                        <div className="mb-8 text-center">
                            <h2 className="text-foreground mb-2 text-2xl font-bold tracking-tight">Get Started</h2>
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
                        <a
                            href="https://github.com/Pontusg45/fubbik"
                            target="_blank"
                            rel="noopener noreferrer"
                            className="text-muted-foreground hover:text-foreground transition-colors"
                        >
                            <Github className="size-4" />
                        </a>
                    </div>
                </footer>
            </div>
        </div>
    );
}
