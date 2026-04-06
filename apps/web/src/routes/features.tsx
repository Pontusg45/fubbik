import { createFileRoute } from "@tanstack/react-router";
import {
    ArrowRight,
    Bot,
    Code2,
    Heart,
    Layers,
    LayoutDashboard,
    Link2,
    Network,
    Sparkles,
    Terminal,
    Zap,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { InstallTabs } from "@/features/landing/install-tabs";

export const Route = createFileRoute("/features")({
    component: FeaturesPage,
});

/* ─── Shared utilities ─── */

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

/* ─── Model flow card ─── */

function ModelCard({
    icon: Icon,
    title,
    description,
}: {
    icon: React.ElementType;
    title: string;
    description: string;
}) {
    return (
        <div className="flex flex-col items-center gap-3 rounded-xl border bg-card p-6 text-center shadow-sm">
            <div className="flex size-12 items-center justify-center rounded-lg bg-primary/10">
                <Icon className="size-6 text-primary" />
            </div>
            <h3 className="font-semibold">{title}</h3>
            <p className="text-sm text-muted-foreground">{description}</p>
        </div>
    );
}

/* ─── Feature list item ─── */

function FeatureItem({ title, description }: { title: string; description: string }) {
    return (
        <div className="space-y-1">
            <h4 className="font-medium text-sm">{title}</h4>
            <p className="text-sm text-muted-foreground">{description}</p>
        </div>
    );
}

/* ─── Surface card ─── */

function SurfaceCard({
    icon: Icon,
    title,
    items,
}: {
    icon: React.ElementType;
    title: string;
    items: string[];
}) {
    return (
        <div className="rounded-xl border bg-card p-6 shadow-sm">
            <div className="mb-4 flex items-center gap-3">
                <div className="flex size-10 items-center justify-center rounded-lg bg-primary/10">
                    <Icon className="size-5 text-primary" />
                </div>
                <h3 className="font-semibold">{title}</h3>
            </div>
            <ul className="space-y-2 text-sm text-muted-foreground">
                {items.map((item) => (
                    <li key={item} className="flex items-start gap-2">
                        <span className="mt-1.5 block size-1.5 shrink-0 rounded-full bg-primary/40" />
                        {item}
                    </li>
                ))}
            </ul>
        </div>
    );
}

/* ─── Health bar ─── */

function HealthBar({ label, value, max }: { label: string; value: number; max: number }) {
    const pct = (value / max) * 100;
    return (
        <div className="space-y-1">
            <div className="flex justify-between text-sm">
                <span className="text-muted-foreground">{label}</span>
                <span className="font-medium">{value}/{max}</span>
            </div>
            <div className="h-2 rounded-full bg-muted">
                <div
                    className="h-2 rounded-full bg-emerald-500/60"
                    style={{ width: `${pct}%` }}
                />
            </div>
        </div>
    );
}

/* ─── Main page ─── */

function FeaturesPage() {
    return (
        <div className="min-h-screen bg-background">
            {/* Header */}
            <FadeInSection>
                <div className="container mx-auto max-w-4xl px-4 py-20 text-center">
                    <h1 className="text-4xl font-bold sm:text-5xl">How Fubbik Works</h1>
                    <p className="mt-4 text-lg text-muted-foreground">
                        The mental model, then the tools.
                    </p>
                </div>
            </FadeInSection>

            {/* Model Diagram */}
            <FadeInSection>
                <section id="model" className="container mx-auto max-w-4xl border-t border-border/40 px-4 py-16">
                    <div className="flex flex-col items-center gap-4 md:flex-row md:gap-2">
                        <ModelCard
                            icon={Layers}
                            title="Chunks"
                            description="Atomic units of knowledge about your codebase."
                        />
                        <ArrowRight className="hidden size-6 shrink-0 text-muted-foreground md:flex" />
                        <ModelCard
                            icon={Link2}
                            title="Connections"
                            description="Typed relationships that form a knowledge graph."
                        />
                        <ArrowRight className="hidden size-6 shrink-0 text-muted-foreground md:flex" />
                        <ModelCard
                            icon={Sparkles}
                            title="Context"
                            description="The right knowledge delivered at the right time."
                        />
                        <ArrowRight className="hidden size-6 shrink-0 text-muted-foreground md:flex" />
                        <ModelCard
                            icon={Zap}
                            title="Action"
                            description="Better decisions, fewer mistakes, faster flow."
                        />
                    </div>
                </section>
            </FadeInSection>

            {/* Chunks Section */}
            <FadeInSection>
                <section id="chunks" className="container mx-auto max-w-4xl border-t border-border/40 px-4 py-16">
                    <p className="text-xs font-semibold uppercase tracking-widest text-muted-foreground">The Model</p>
                    <h2 className="mt-2 text-3xl font-bold">Atomic units of knowledge</h2>
                    <p className="mt-4 max-w-2xl text-muted-foreground">
                        A chunk is one discrete piece of knowledge: a convention, an architecture decision,
                        a runbook, a schema definition. Small enough to be precise, rich enough to be useful.
                    </p>

                    {/* Example chunk card */}
                    <div className="mt-8 rounded-xl border bg-card p-6 shadow-sm">
                        <div className="flex flex-wrap items-center gap-2">
                            <Badge variant="secondary" size="sm">Convention</Badge>
                            <span className="font-semibold">Always use Effect for typed errors</span>
                        </div>
                        <div className="mt-3 flex flex-wrap gap-1.5">
                            <Badge variant="outline" size="sm">#backend</Badge>
                            <Badge variant="outline" size="sm">#error-handling</Badge>
                        </div>
                        <p className="mt-3 text-sm text-muted-foreground line-clamp-2">
                            All service-layer functions must return Effect types with tagged errors.
                            Use Effect.tryPromise for async operations and map database errors to
                            typed DatabaseError instances...
                        </p>
                        <div className="mt-4 flex flex-wrap items-center gap-4 text-xs text-muted-foreground">
                            <span className="flex items-center gap-1">
                                <Heart className="size-3.5 text-emerald-500" />
                                Health: 82/100
                            </span>
                            <span>3 connections</span>
                            <span>Updated 5 days ago</span>
                        </div>
                    </div>

                    {/* Feature list */}
                    <div className="mt-10 grid gap-6 sm:grid-cols-2">
                        <FeatureItem
                            title="Types"
                            description="Note, document, reference, schema, or checklist. Each type shapes how the chunk is displayed and used."
                        />
                        <FeatureItem
                            title="Tags"
                            description="Freeform tags with optional tag types for structured categorization and filtering."
                        />
                        <FeatureItem
                            title="Scope & appliesTo"
                            description="JSONB metadata and glob patterns that link chunks to file areas in your codebase."
                        />
                        <FeatureItem
                            title="File references"
                            description="Explicit bidirectional links to specific files and symbols for precise traceability."
                        />
                        <FeatureItem
                            title="Decision context"
                            description="Optional rationale, alternatives, and consequences fields capture the 'why' behind decisions."
                        />
                        <FeatureItem
                            title="Version history"
                            description="Append-only history tracks every change, so knowledge evolution is never lost."
                        />
                        <FeatureItem
                            title="Templates"
                            description="Built-in and custom templates for conventions, architecture decisions, runbooks, and more."
                        />
                        <FeatureItem
                            title="AI enrichment"
                            description="Automatic summaries, aliases, and embeddings via local Ollama models."
                        />
                    </div>
                </section>
            </FadeInSection>

            {/* Connections Section */}
            <FadeInSection>
                <section id="connections" className="container mx-auto max-w-4xl border-t border-border/40 px-4 py-16">
                    <h2 className="text-3xl font-bold">Typed relationships between knowledge</h2>
                    <p className="mt-4 max-w-2xl text-muted-foreground">
                        Connections are directed edges between chunks. They form a knowledge graph that
                        surfaces dependencies, contradictions, and related context automatically.
                    </p>

                    {/* Example connections */}
                    <div className="mt-8 space-y-3 rounded-xl border bg-card p-6 shadow-sm font-mono text-sm">
                        <p>Auth Middleware <span className="text-muted-foreground">&rarr;</span> <Badge variant="secondary" size="sm">depends_on</Badge> <span className="text-muted-foreground">&rarr;</span> Session Token Format</p>
                        <p>Error Handling Guide <span className="text-muted-foreground">&rarr;</span> <Badge variant="secondary" size="sm">part_of</Badge> <span className="text-muted-foreground">&rarr;</span> Backend Conventions</p>
                        <p>REST API Style <span className="text-muted-foreground">&rarr;</span> <Badge variant="secondary" size="sm">contradicts</Badge> <span className="text-muted-foreground">&rarr;</span> GraphQL Migration Plan</p>
                        <p>Drizzle ORM Setup <span className="text-muted-foreground">&rarr;</span> <Badge variant="secondary" size="sm">extends</Badge> <span className="text-muted-foreground">&rarr;</span> Database Schema Guide</p>
                    </div>

                    {/* Relation types */}
                    <div className="mt-6 flex flex-wrap gap-2">
                        {["depends_on", "part_of", "extends", "references", "supports", "contradicts", "alternative_to", "related_to"].map((rel) => (
                            <Badge key={rel} variant="outline" size="sm">{rel}</Badge>
                        ))}
                    </div>

                    {/* Feature list */}
                    <div className="mt-10 grid gap-6 sm:grid-cols-2">
                        <FeatureItem
                            title="Dependency tree"
                            description="Visualize transitive dependencies grouped by relation type on any chunk."
                        />
                        <FeatureItem
                            title="Related suggestions"
                            description="Embedding-based similarity finds chunks that should be connected."
                        />
                        <FeatureItem
                            title="Relation picker"
                            description="Quick UI for creating connections with the right relation type."
                        />
                        <FeatureItem
                            title="Cross-codebase"
                            description="Connections are global, enabling knowledge linking across projects."
                        />
                        <FeatureItem
                            title="Graph focus mode"
                            description="Double-click any node to explore its neighborhood in isolation."
                        />
                        <FeatureItem
                            title="Filter presets"
                            description="Save and reuse graph filter configurations for common views."
                        />
                    </div>
                </section>
            </FadeInSection>

            {/* Context Section */}
            <FadeInSection>
                <section id="context" className="container mx-auto max-w-4xl border-t border-border/40 px-4 py-16">
                    <h2 className="text-3xl font-bold">The right knowledge at the right time</h2>
                    <p className="mt-4 max-w-2xl text-muted-foreground">
                        Context is the bridge between stored knowledge and active work. Fubbik matches
                        chunks to your current file, search query, or AI session automatically.
                    </p>

                    {/* Context flow diagram */}
                    <div className="mt-8 grid gap-0 sm:grid-cols-3">
                        <div className="flex flex-col items-center border-dashed p-6 sm:border-r">
                            <div className="flex size-10 items-center justify-center rounded-full bg-primary/10 text-lg font-bold text-primary">1</div>
                            <h4 className="mt-3 font-semibold">File path</h4>
                            <p className="mt-1 text-center text-sm text-muted-foreground">
                                You open a file or ask about a path.
                            </p>
                        </div>
                        <div className="flex flex-col items-center border-dashed p-6 sm:border-r">
                            <div className="flex size-10 items-center justify-center rounded-full bg-primary/10 text-lg font-bold text-primary">2</div>
                            <h4 className="mt-3 font-semibold">Match</h4>
                            <p className="mt-1 text-center text-sm text-muted-foreground">
                                File refs, globs, and embeddings find relevant chunks.
                            </p>
                        </div>
                        <div className="flex flex-col items-center p-6">
                            <div className="flex size-10 items-center justify-center rounded-full bg-primary/10 text-lg font-bold text-primary">3</div>
                            <h4 className="mt-3 font-semibold">Deliver</h4>
                            <p className="mt-1 text-center text-sm text-muted-foreground">
                                Context appears in your editor, CLI, or AI session.
                            </p>
                        </div>
                    </div>

                    {/* Feature list */}
                    <div className="mt-10 grid gap-6 sm:grid-cols-2">
                        <FeatureItem
                            title="File-aware context"
                            description="Chunks matched via file references, appliesTo globs, and dependency analysis."
                        />
                        <FeatureItem
                            title="Semantic search"
                            description="Vector embeddings find conceptually related chunks beyond keyword matching."
                        />
                        <FeatureItem
                            title="Federated search"
                            description="Search across all codebases with results grouped by project."
                        />
                        <FeatureItem
                            title="Token-budgeted export"
                            description="Context export respects token limits, prioritizing by health and relevance."
                        />
                        <FeatureItem
                            title="CLAUDE.md generation"
                            description="Auto-generate project context files from tagged chunks for AI assistants."
                        />
                        <FeatureItem
                            title="MCP server"
                            description="Model Context Protocol integration gives AI agents direct access to your knowledge."
                        />
                        <FeatureItem
                            title="Context-for-file API"
                            description="Programmatic endpoint returns chunks relevant to any file path with match reasons."
                        />
                    </div>
                </section>
            </FadeInSection>

            {/* Health Section */}
            <FadeInSection>
                <section id="health" className="container mx-auto max-w-4xl border-t border-border/40 px-4 py-16">
                    <h2 className="text-3xl font-bold">Knowledge that maintains itself</h2>
                    <p className="mt-4 max-w-2xl text-muted-foreground">
                        Every chunk has a health score computed from freshness, completeness, richness,
                        and connectivity. Stale or thin knowledge surfaces automatically so you can fix it.
                    </p>

                    {/* Health score breakdown */}
                    <div className="mt-8 rounded-xl border bg-card p-6 shadow-sm">
                        <div className="mb-4 flex items-center justify-between">
                            <h3 className="font-semibold">Health Score Breakdown</h3>
                            <span className="text-lg font-bold text-emerald-500">88/100</span>
                        </div>
                        <div className="space-y-3">
                            <HealthBar label="Freshness" value={20} max={25} />
                            <HealthBar label="Completeness" value={25} max={25} />
                            <HealthBar label="Richness" value={18} max={25} />
                            <HealthBar label="Connectivity" value={25} max={25} />
                        </div>
                    </div>

                    {/* Staleness banner */}
                    <div className="mt-4 flex items-start gap-3 rounded-xl border border-amber-500/40 bg-card p-4">
                        <Heart className="mt-0.5 size-5 shrink-0 text-amber-500" />
                        <div>
                            <p className="text-sm font-medium">3 files changed since this chunk was last updated</p>
                            <p className="mt-1 text-xs text-muted-foreground">
                                src/auth/middleware.ts, src/auth/session.ts, and 1 more file were modified.
                                Consider reviewing this chunk for accuracy.
                            </p>
                        </div>
                    </div>

                    {/* Feature list */}
                    <div className="mt-10 grid gap-6 sm:grid-cols-2">
                        <FeatureItem
                            title="Freshness tracking"
                            description="Scores decay over time, flagging knowledge that may be outdated."
                        />
                        <FeatureItem
                            title="Completeness checks"
                            description="Chunks with rationale, alternatives, and consequences score higher."
                        />
                        <FeatureItem
                            title="Staleness detection"
                            description="Git-aware scanning detects when referenced files change after a chunk was written."
                        />
                        <FeatureItem
                            title="Knowledge health dashboard"
                            description="Orphaned, stale, and thin chunks surfaced in one place for triage."
                        />
                        <FeatureItem
                            title="Embedding freshness"
                            description="Tracks when embeddings were last refreshed and flags stale vectors."
                        />
                        <FeatureItem
                            title="Attention needed"
                            description="Dashboard widget and nav badge highlight chunks that need review."
                        />
                    </div>
                </section>
            </FadeInSection>

            {/* Surfaces Section */}
            <FadeInSection>
                <section id="surfaces" className="container mx-auto max-w-4xl border-t border-border/40 px-4 py-16">
                    <h2 className="text-3xl font-bold">Where you interact</h2>
                    <p className="mt-2 text-lg text-muted-foreground">Five surfaces, one knowledge base.</p>

                    <div className="mt-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                        <SurfaceCard
                            icon={LayoutDashboard}
                            title="Web UI"
                            items={[
                                "Dashboard with health overview",
                                "Knowledge graph visualization",
                                "Chunk editor with templates",
                                "Search and filter across codebases",
                                "Plan and requirement tracking",
                            ]}
                        />
                        <SurfaceCard
                            icon={Terminal}
                            title="CLI"
                            items={[
                                "CRUD with colored output",
                                "Context export for AI sessions",
                                "CLAUDE.md generation",
                                "Git hook integration",
                                "Plan management",
                            ]}
                        />
                        <SurfaceCard
                            icon={Code2}
                            title="VS Code"
                            items={[
                                "Sidebar with file-aware chunks",
                                "Inline chunk creation",
                                "Search and quick-add",
                                "Status bar chunk count",
                                "Open graph in browser",
                            ]}
                        />
                        <SurfaceCard
                            icon={Bot}
                            title="MCP Server"
                            items={[
                                "Tools for AI agent integration",
                                "Plan creation and tracking",
                                "Implementation sessions",
                                "Context retrieval",
                                "Requirement management",
                            ]}
                        />
                        <SurfaceCard
                            icon={Network}
                            title="API"
                            items={[
                                "Full REST API with OpenAPI docs",
                                "Semantic and federated search",
                                "Token-budgeted context export",
                                "Bulk import and export",
                                "Webhook-friendly endpoints",
                            ]}
                        />
                    </div>
                </section>
            </FadeInSection>

            {/* Bottom CTA */}
            <FadeInSection>
                <section className="container mx-auto max-w-4xl border-t border-border/40 px-4 py-16 text-center">
                    <h2 className="text-3xl font-bold">Ready to try it?</h2>
                    <div className="mt-8">
                        <InstallTabs />
                    </div>
                </section>
            </FadeInSection>
        </div>
    );
}
