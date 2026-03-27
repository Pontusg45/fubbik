import { useState } from "react";
import { Link } from "@tanstack/react-router";
import { Blocks, GitBranch, Network, Sparkles } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";

export function WelcomeWizard() {
    const [step, setStep] = useState(0);

    const steps = [
        {
            icon: GitBranch,
            title: "Set up your codebase",
            description: "Register your project so chunks are scoped to it.",
            action: (
                <Button size="sm" render={<Link to="/codebases" />}>
                    Add Codebase
                </Button>
            )
        },
        {
            icon: Blocks,
            title: "Create your first chunk",
            description:
                "Document a convention, architecture decision, or runbook.",
            action: (
                <Button size="sm" render={<Link to="/chunks/new" />}>
                    Create Chunk
                </Button>
            )
        },
        {
            icon: Network,
            title: "Explore the graph",
            description: "See how your knowledge connects visually.",
            action: (
                <Button size="sm" render={<Link to="/graph" />}>
                    Open Graph
                </Button>
            )
        },
        {
            icon: Sparkles,
            title: "Enrich with AI",
            description:
                "Let Ollama generate summaries, aliases, and embeddings.",
            action: (
                <Button size="sm" variant="outline" render={<Link to="/codebases" />}>
                    Configure AI
                </Button>
            )
        }
    ];

    return (
        <div className="mx-auto max-w-2xl py-12">
            <div className="mb-8 text-center">
                <h1 className="text-3xl font-bold tracking-tight">
                    Welcome to fubbik
                </h1>
                <p className="text-muted-foreground mt-2">
                    Your local-first knowledge framework. Let's get started.
                </p>
            </div>
            <div className="space-y-3">
                {steps.map((s, i) => (
                    <Card key={i}>
                        <CardPanel
                            className={`flex items-center gap-4 p-4 ${i <= step ? "" : "opacity-50"}`}
                        >
                            <div
                                className={`rounded-full p-2 ${
                                    i < step
                                        ? "bg-green-500/10 text-green-600"
                                        : i === step
                                          ? "bg-primary/10 text-primary"
                                          : "bg-muted text-muted-foreground"
                                }`}
                            >
                                <s.icon className="size-5" />
                            </div>
                            <div className="flex-1">
                                <p className="text-sm font-medium">{s.title}</p>
                                <p className="text-muted-foreground text-xs">
                                    {s.description}
                                </p>
                            </div>
                            {i === step && s.action}
                            {i < step && (
                                <span className="text-xs font-medium text-green-600">
                                    Done
                                </span>
                            )}
                        </CardPanel>
                    </Card>
                ))}
            </div>
            <div className="mt-6 flex items-center justify-center gap-4">
                {step > 0 && (
                    <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => setStep(s => s - 1)}
                    >
                        Back
                    </Button>
                )}
                {step < steps.length - 1 && (
                    <Button
                        variant="outline"
                        size="sm"
                        onClick={() => setStep(s => s + 1)}
                    >
                        Skip this step
                    </Button>
                )}
            </div>
            <p className="text-muted-foreground mt-6 text-center text-xs">
                You can also import chunks via CLI:{" "}
                <code className="bg-muted rounded px-1">fubbik add -i</code>
            </p>
        </div>
    );
}
