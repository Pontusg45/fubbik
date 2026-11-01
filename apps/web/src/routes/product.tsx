import { createFileRoute, Link } from "@tanstack/react-router";
import { ArrowRight, Bot, HeartPulse, Network, PencilLine } from "lucide-react";

export const Route = createFileRoute("/product")({ component: ProductPage });

const sections = [
    {
        id: "chunks",
        icon: Network,
        title: "Knowledge Graph",
        text: "Store decisions, notes, concepts, and code references as typed chunks. Connect related knowledge and follow those links in the graph.",
        action: "Explore the graph",
        to: "/graph" as const
    },
    {
        id: "health",
        icon: HeartPulse,
        title: "Health Monitoring",
        text: "Find stale, thin, disconnected, and uncovered knowledge before it becomes hard to trust.",
        action: "View knowledge health",
        to: "/knowledge-health" as const
    },
    {
        id: "context",
        icon: Bot,
        title: "AI-Native Context",
        text: "Find the chunks relevant to a file or task, then prepare focused context for coding tools and agents.",
        action: "Find context",
        to: "/context" as const
    },
    {
        id: "connections",
        icon: PencilLine,
        title: "Capture & Create",
        text: "Create knowledge from the web app, CLI, editor, or API and keep it connected to the rest of your codebase.",
        action: "Create a chunk",
        to: "/chunks/new" as const
    }
];

function ProductPage() {
    return (
        <div className="container mx-auto max-w-5xl px-4 py-12">
            <div className="mb-10 max-w-2xl">
                <p className="text-primary mb-2 text-sm font-semibold">How it works</p>
                <h1 className="text-4xl font-bold tracking-tight">Knowledge that stays connected to your code</h1>
                <p className="text-muted-foreground mt-4 text-lg">
                    Capture what your team knows, explore the relationships, and find what needs attention.
                </p>
            </div>
            <nav aria-label="Product sections" className="mb-10 flex flex-wrap gap-2">
                {sections.map(section => (
                    <a key={section.id} href={`#${section.id}`} className="hover:bg-muted rounded-md border px-3 py-2 text-sm">
                        {section.title}
                    </a>
                ))}
            </nav>
            <div className="space-y-6">
                {sections.map(section => (
                    <section key={section.id} id={section.id} className="scroll-mt-6 rounded-xl border p-6 sm:p-8">
                        <section.icon className="text-primary mb-4 size-7" />
                        <h2 className="text-2xl font-semibold">{section.title}</h2>
                        <p className="text-muted-foreground mt-2 max-w-2xl">{section.text}</p>
                        <Link
                            to={section.to}
                            className="text-primary mt-5 inline-flex items-center gap-1 text-sm font-medium hover:underline"
                        >
                            {section.action}
                            <ArrowRight className="size-4" />
                        </Link>
                    </section>
                ))}
            </div>
        </div>
    );
}
