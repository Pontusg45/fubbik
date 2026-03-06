import { useMutation, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { ArrowLeft } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardPanel } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { api } from "@/utils/api";

export const Route = createFileRoute("/chunks/new")({
  component: NewChunk,
});

function NewChunk() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [type, setType] = useState("note");
  const [tagInput, setTagInput] = useState("");
  const [tags, setTags] = useState<string[]>([]);

  const createMutation = useMutation({
    mutationFn: async () => {
      const { data, error } = await api.api.chunks.post({
        title,
        content,
        type,
        tags,
      });
      if (error) throw new Error("Failed to create chunk");
      return data;
    },
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ["chunks"] });
      queryClient.invalidateQueries({ queryKey: ["stats"] });
      toast.success("Chunk created");
      if (data && "id" in data) {
        navigate({ to: "/chunks/$chunkId", params: { chunkId: data.id } });
      }
    },
    onError: () => {
      toast.error("Failed to create chunk");
    },
  });

  const addTag = () => {
    const tag = tagInput.trim().toLowerCase();
    if (tag && !tags.includes(tag)) {
      setTags([...tags, tag]);
    }
    setTagInput("");
  };

  return (
    <div className="container mx-auto max-w-3xl px-4 py-8">
      <div className="mb-6 flex items-center justify-between">
        <Button variant="ghost" size="sm" render={<Link to="/dashboard" />}>
          <ArrowLeft className="size-4" />
          Back
        </Button>
      </div>

      <h1 className="text-2xl font-bold tracking-tight mb-6">New Chunk</h1>

      <Card>
        <CardPanel className="space-y-4 p-6">
          <div>
            <label className="text-sm font-medium mb-1.5 block">Title</label>
            <input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Enter a title..."
              className="bg-background border rounded-md px-3 py-2 text-sm w-full focus:outline-none focus:ring-2 focus:ring-ring"
            />
          </div>

          <div>
            <label className="text-sm font-medium mb-1.5 block">Type</label>
            <select
              value={type}
              onChange={(e) => setType(e.target.value)}
              className="bg-background border rounded-md px-3 py-2 text-sm w-full focus:outline-none focus:ring-2 focus:ring-ring"
            >
              <option value="note">Note</option>
              <option value="document">Document</option>
              <option value="reference">Reference</option>
              <option value="schema">Schema</option>
              <option value="checklist">Checklist</option>
            </select>
          </div>

          <div>
            <label className="text-sm font-medium mb-1.5 block">Tags</label>
            <div className="flex gap-2 mb-2 flex-wrap">
              {tags.map((tag) => (
                <Badge
                  key={tag}
                  variant="secondary"
                  size="sm"
                  className="cursor-pointer"
                  onClick={() => setTags(tags.filter((t) => t !== tag))}
                >
                  {tag} ×
                </Badge>
              ))}
            </div>
            <input
              type="text"
              value={tagInput}
              onChange={(e) => setTagInput(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  addTag();
                }
              }}
              placeholder="Add a tag and press Enter..."
              className="bg-background border rounded-md px-3 py-2 text-sm w-full focus:outline-none focus:ring-2 focus:ring-ring"
            />
          </div>

          <div>
            <label className="text-sm font-medium mb-1.5 block">Content</label>
            <textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder="Write your content..."
              rows={10}
              className="bg-background border rounded-md px-3 py-2 text-sm w-full focus:outline-none focus:ring-2 focus:ring-ring resize-y"
            />
          </div>

          <Separator />

          <div className="flex justify-end gap-2">
            <Button variant="outline" render={<Link to="/dashboard" />}>
              Cancel
            </Button>
            <Button
              onClick={() => createMutation.mutate()}
              disabled={!title.trim() || createMutation.isPending}
            >
              {createMutation.isPending ? "Creating..." : "Create Chunk"}
            </Button>
          </div>
        </CardPanel>
      </Card>
    </div>
  );
}
