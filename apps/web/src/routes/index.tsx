import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
  Blocks,
  Bot,
  ChevronRight,
  Copy,
  Check,
  HardDrive,
  History,
  Network,
  Tags,
  Terminal,
  MousePointer2,
} from "lucide-react";
import { useState } from "react";

import FubbikLogo from "@/components/fubbik-logo";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardDescription,
  CardHeader,
  CardPanel,
  CardTitle,
} from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTab } from "@/components/ui/tabs";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { api } from "@/utils/api";

export const Route = createFileRoute("/")({
  component: HomeComponent,
});

const features = [
  {
    icon: Blocks,
    title: "Chunk-Based Storage",
    description:
      "Self-contained units of information that reference each other, tagged and typed. No rigid schemas — just content with shape.",
  },
  {
    icon: Bot,
    title: "AI & Human Friendly",
    description:
      "A CLI optimized for programmatic and AI-agent access. A GUI built for human exploration. Both first-class.",
  },
  {
    icon: HardDrive,
    title: "Local First",
    description:
      "Your data lives on your machine. No accounts, no cloud sync required. Fully offline, portable, and yours.",
  },
  {
    icon: History,
    title: "Transaction Logs",
    description:
      "Every change is recorded in an append-only log. Audit history, replay changes, or roll back to any previous state.",
  },
  {
    icon: Tags,
    title: "Dynamic Metadata",
    description:
      "Add, remove, or reshape metadata fields at any time without migrations. Your data model evolves with your needs.",
  },
  {
    icon: Network,
    title: "Knowledge Graphs",
    description:
      "Relationships between chunks form a graph you can see. Explore connections and navigate your knowledge spatially.",
  },
] as const;

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);

  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            variant="ghost"
            size="icon-xs"
            onClick={() => {
              navigator.clipboard.writeText(text);
              setCopied(true);
              setTimeout(() => setCopied(false), 2000);
            }}
          />
        }
      >
        {copied ? (
          <Check className="size-3 text-green-500" />
        ) : (
          <Copy className="size-3" />
        )}
      </TooltipTrigger>
      <TooltipContent>{copied ? "Copied!" : "Copy"}</TooltipContent>
    </Tooltip>
  );
}

function InstallBlock() {
  return (
    <div className="mt-8">
      <Tabs defaultValue="bun">
        <TabsList>
          <TabsTab value="bun">bun</TabsTab>
          <TabsTab value="cli">CLI</TabsTab>
          <TabsTab value="docker">Docker</TabsTab>
        </TabsList>
        <TabsContent value="bun">
          <CodeBlock code="bun install && bun dev" />
        </TabsContent>
        <TabsContent value="cli">
          <CodeBlock code="fubbik init my-knowledge-base" />
        </TabsContent>
        <TabsContent value="docker">
          <CodeBlock code="docker compose up" />
        </TabsContent>
      </Tabs>
    </div>
  );
}

function CodeBlock({ code }: { code: string }) {
  return (
    <div className="bg-muted/50 mt-2 flex items-center justify-between gap-4 rounded-lg border px-4 py-3 font-mono text-sm">
      <div className="flex items-center gap-2 overflow-x-auto">
        <Terminal className="text-muted-foreground size-3.5 shrink-0" />
        <code>{code}</code>
      </div>
      <CopyButton text={code} />
    </div>
  );
}

function FeatureCard({
  feature,
  index,
}: {
  feature: (typeof features)[number];
  index: number;
}) {
  const [hovered, setHovered] = useState(false);

  return (
    <Card
      className={`transition-all duration-200 ${hovered ? "border-foreground/20 -translate-y-0.5 shadow-md" : ""}`}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <CardHeader>
        <div className="mb-1 flex items-center gap-2">
          <feature.icon
            className={`size-4 transition-colors duration-200 ${hovered ? "text-foreground" : "text-muted-foreground"}`}
          />
          <Badge variant="secondary" size="sm" className="font-mono">
            {String(index + 1).padStart(2, "0")}
          </Badge>
        </div>
        <CardTitle className="font-mono text-base">{feature.title}</CardTitle>
        <CardDescription>{feature.description}</CardDescription>
      </CardHeader>
      <CardPanel className="pt-0">
        <div
          className={`flex items-center gap-1 text-xs transition-opacity duration-200 ${hovered ? "opacity-100" : "opacity-0"}`}
        >
          <span className="text-muted-foreground">Learn more</span>
          <ChevronRight className="text-muted-foreground size-3" />
        </div>
      </CardPanel>
    </Card>
  );
}

function ApiStatus() {
  const healthCheck = useQuery({
    queryKey: ["health"],
    queryFn: async () => {
      const { data } = await api.api.health.get();
      return data;
    },
  });

  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <button className="inline-flex items-center gap-2 rounded-lg border px-3 py-1.5 font-mono text-xs transition-colors hover:bg-muted/50" />
        }
      >
        <span className="relative flex size-2">
          <span
            className={`absolute inline-flex size-full animate-ping rounded-full opacity-75 ${healthCheck.data ? "bg-green-400" : healthCheck.isLoading ? "bg-yellow-400" : "bg-red-400"}`}
          />
          <span
            className={`relative inline-flex size-2 rounded-full ${healthCheck.data ? "bg-green-500" : healthCheck.isLoading ? "bg-yellow-500" : "bg-red-500"}`}
          />
        </span>
        <span className="text-muted-foreground">API</span>
      </TooltipTrigger>
      <TooltipContent>
        {healthCheck.isLoading
          ? "Checking connection..."
          : healthCheck.data
            ? "API is connected"
            : "API is disconnected"}
      </TooltipContent>
    </Tooltip>
  );
}

function HomeComponent() {
  return (
    <TooltipProvider>
      <div className="container mx-auto max-w-4xl px-4 py-16">
        <div className="mb-16 flex items-center justify-between gap-12">
          <div className="flex flex-col items-start gap-6">
            <div className="flex items-center gap-3">
              <Badge variant="outline" className="font-mono text-xs">
                Proof of Concept
              </Badge>
              <ApiStatus />
            </div>

            <h1 className="text-4xl font-bold tracking-tight">fubbik</h1>
            <p className="text-muted-foreground max-w-lg text-sm leading-relaxed">
              A local-first knowledge framework for humans and machines. Store,
              navigate, and evolve structured knowledge as discrete chunks — each
              with its own metadata, history, and relationships.
            </p>

            <div className="flex gap-2">
              <Button size="lg" render={<a href="/dashboard" />}>
                Get Started
                <ChevronRight className="size-4" />
              </Button>
              <Button
                variant="outline"
                size="lg"
                render={
                  <a
                    href="https://github.com/pontusg45/fubbik"
                    target="_blank"
                    rel="noopener noreferrer"
                  />
                }
              >
                GitHub
              </Button>
            </div>

            <InstallBlock />
          </div>

          <FubbikLogo className="hidden size-48 shrink-0 sm:block" />
        </div>

        <div className="mb-4 flex items-center gap-2">
          <MousePointer2 className="text-muted-foreground size-3.5" />
          <span className="text-muted-foreground text-xs">
            Hover to explore features
          </span>
        </div>

        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {features.map((feature, i) => (
            <FeatureCard key={feature.title} feature={feature} index={i} />
          ))}
        </div>

        <div className="text-muted-foreground mt-16 border-t pt-4 font-mono text-xs">
          Built with TanStack Start · Elysia · Eden · Drizzle · Better Auth
        </div>
      </div>
    </TooltipProvider>
  );
}
