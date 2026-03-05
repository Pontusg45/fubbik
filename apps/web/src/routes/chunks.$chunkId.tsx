import { createFileRoute, Link } from "@tanstack/react-router";
import { ArrowLeft, Calendar, Clock, Edit, GitBranch, Hash, Network, Trash2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardHeader, CardPanel, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";

export const Route = createFileRoute("/chunks/$chunkId")({
  component: ChunkDetail,
});

const mockChunks: Record<
  string,
  {
    id: string;
    title: string;
    type: string;
    tags: string[];
    created: string;
    updated: string;
    content: string;
    connections: { id: string; title: string; relation: string }[];
    history: { action: string; timestamp: string }[];
  }
> = {
  "c-001": {
    id: "c-001",
    title: "Project Architecture Notes",
    type: "document",
    tags: ["architecture", "planning"],
    created: "2026-02-28",
    updated: "2 min ago",
    content:
      "The system follows a modular monorepo structure with clear separation between apps and packages.\n\n## Key decisions\n\n- **Runtime**: Bun for speed and native TypeScript support\n- **API**: Elysia with Eden treaty for end-to-end type safety\n- **Database**: PostgreSQL with Drizzle ORM\n- **Auth**: Better Auth with session-based authentication\n\n## Package layout\n\n- `packages/api` — shared API definition\n- `packages/auth` — authentication logic\n- `packages/db` — schema and migrations\n- `apps/web` — TanStack Start frontend\n- `apps/server` — Elysia backend\n- `apps/cli` — Commander.js CLI tool",
    connections: [
      { id: "c-002", title: "API Design Patterns", relation: "references" },
      { id: "c-004", title: "Database Schema v2", relation: "depends on" },
    ],
    history: [
      { action: "Updated content", timestamp: "2 min ago" },
      { action: "Added tag: planning", timestamp: "1 hour ago" },
      { action: "Created", timestamp: "2026-02-28" },
    ],
  },
  "c-002": {
    id: "c-002",
    title: "API Design Patterns",
    type: "reference",
    tags: ["api", "patterns"],
    created: "2026-03-01",
    updated: "1 hour ago",
    content:
      "A collection of patterns used in the API layer.\n\n## Eden Treaty\n\nType-safe client generated from the Elysia server definition. No code generation step needed — types flow directly.\n\n## Error handling\n\nAll endpoints return `{ data, error }` via Eden. Errors include status codes and messages.\n\n## Authentication\n\nSession resolution happens in a shared `resolve` middleware. Protected routes check `session` and return 401 if missing.",
    connections: [{ id: "c-001", title: "Project Architecture Notes", relation: "referenced by" }],
    history: [
      { action: "Updated content", timestamp: "1 hour ago" },
      { action: "Created", timestamp: "2026-03-01" },
    ],
  },
};

function ChunkDetail() {
  const { chunkId } = Route.useParams();
  const chunk = mockChunks[chunkId];

  if (!chunk) {
    return (
      <div className="container mx-auto max-w-3xl px-4 py-8">
        <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
          <ArrowLeft className="size-4" />
          Back
        </Button>
        <div className="mt-8 text-center">
          <p className="text-muted-foreground">Chunk "{chunkId}" not found.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="container mx-auto max-w-3xl px-4 py-8">
      <div className="mb-6 flex items-center justify-between">
        <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
          <ArrowLeft className="size-4" />
          Back
        </Button>
        <div className="flex gap-2">
          <Button variant="outline" size="sm">
            <Edit className="size-3.5" />
            Edit
          </Button>
          <Button variant="outline" size="sm" className="text-destructive">
            <Trash2 className="size-3.5" />
            Delete
          </Button>
        </div>
      </div>

      <div className="mb-6">
        <div className="mb-2 flex items-center gap-2">
          <Badge variant="secondary" className="font-mono text-xs">
            {chunk.type}
          </Badge>
          <span className="text-muted-foreground font-mono text-xs flex items-center gap-1">
            <Hash className="size-3" />
            {chunk.id}
          </span>
        </div>
        <h1 className="text-2xl font-bold tracking-tight">{chunk.title}</h1>
        <div className="text-muted-foreground mt-2 flex items-center gap-4 text-xs">
          <span className="flex items-center gap-1">
            <Calendar className="size-3" />
            Created {chunk.created}
          </span>
          <span className="flex items-center gap-1">
            <Clock className="size-3" />
            Updated {chunk.updated}
          </span>
        </div>
      </div>

      <div className="mb-2 flex flex-wrap gap-1.5">
        {chunk.tags.map((tag) => (
          <Badge key={tag} variant="outline" size="sm">
            {tag}
          </Badge>
        ))}
      </div>

      <Separator className="my-6" />

      <div className="prose prose-invert prose-sm max-w-none">
        {chunk.content.split("\n\n").map((block) => {
          if (block.startsWith("## ")) {
            return (
              <h2 key={block} className="mt-6 mb-2 text-base font-semibold">
                {block.replace("## ", "")}
              </h2>
            );
          }
          if (block.startsWith("- ")) {
            return (
              <ul key={block} className="text-muted-foreground list-disc space-y-1 pl-5 text-sm">
                {block.split("\n").map((line) => (
                  <li
                    key={line}
                    dangerouslySetInnerHTML={{
                      __html: line
                        .replace(/^- /, "")
                        .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
                        .replace(
                          /`(.+?)`/g,
                          "<code class='bg-muted rounded px-1 py-0.5 font-mono text-xs'>$1</code>",
                        ),
                    }}
                  />
                ))}
              </ul>
            );
          }
          return (
            <p key={block} className="text-muted-foreground text-sm leading-relaxed">
              {block}
            </p>
          );
        })}
      </div>

      <Separator className="my-6" />

      <div className="grid gap-6 sm:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-sm">
              <Network className="size-4" />
              Connections
            </CardTitle>
            <CardDescription>{chunk.connections.length} linked chunks</CardDescription>
          </CardHeader>
          <CardPanel className="space-y-2 pt-0">
            {chunk.connections.map((conn) => (
              <Link
                key={conn.id}
                to="/chunks/$chunkId"
                params={{ chunkId: conn.id }}
                className="hover:bg-muted flex items-center justify-between rounded-md border px-3 py-2 text-sm transition-colors"
              >
                <span className="font-medium">{conn.title}</span>
                <Badge variant="outline" size="sm" className="text-[10px]">
                  {conn.relation}
                </Badge>
              </Link>
            ))}
          </CardPanel>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-sm">
              <GitBranch className="size-4" />
              History
            </CardTitle>
            <CardDescription>{chunk.history.length} events</CardDescription>
          </CardHeader>
          <CardPanel className="space-y-2 pt-0">
            {chunk.history.map((event) => (
              <div
                key={`${event.action}-${event.timestamp}`}
                className="flex items-center justify-between text-sm"
              >
                <span className="text-muted-foreground">{event.action}</span>
                <span className="text-muted-foreground text-xs">{event.timestamp}</span>
              </div>
            ))}
          </CardPanel>
        </Card>
      </div>
    </div>
  );
}
