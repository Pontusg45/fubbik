import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { X, Plus } from "lucide-react";
import { api } from "@/utils/api";
import { unwrapEden } from "@/utils/eden";

interface InlineTagEditorProps {
  tags: Array<{ id: string; name: string }>;
  onUpdate: (tagNames: string[]) => void;
  loading?: boolean;
}

export function InlineTagEditor({ tags, onUpdate, loading }: InlineTagEditorProps) {
  const [editing, setEditing] = useState(false);
  const [input, setInput] = useState("");
  const [highlightedIndex, setHighlightedIndex] = useState(-1);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const tagNames = tags.map((t) => t.name);

  const { data: allTags } = useQuery({
    queryKey: ["tags"],
    queryFn: async () => unwrapEden(await api.api.tags.get()) as Array<{ id: string; name: string }>,
    enabled: editing,
    staleTime: 30_000,
  });

  const filteredSuggestions = (allTags ?? []).filter(
    (t) =>
      t.name.toLowerCase().includes(input.toLowerCase()) &&
      !tagNames.includes(t.name) &&
      input.trim().length > 0
  );

  const addTag = (name?: string) => {
    const tag = (name ?? input.trim()).toLowerCase();
    if (tag && !tagNames.includes(tag)) {
      onUpdate([...tagNames, tag]);
    }
    setInput("");
    setHighlightedIndex(-1);
  };

  const removeTag = (name: string) => {
    onUpdate(tagNames.filter((t) => t !== name));
  };

  const selectSuggestion = (name: string) => {
    addTag(name);
  };

  // Reset highlight when suggestions change
  useEffect(() => {
    setHighlightedIndex(-1);
  }, [input]);

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {tags.map((tag) => (
        <Badge key={tag.id} variant="secondary" className="gap-1">
          {tag.name}
          <button
            onClick={() => removeTag(tag.name)}
            className="hover:text-destructive ml-0.5"
            disabled={loading}
          >
            <X className="h-3 w-3" />
          </button>
        </Badge>
      ))}
      {editing ? (
        <div className="relative">
          <Input
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setHighlightedIndex((i) =>
                  i < filteredSuggestions.length - 1 ? i + 1 : i
                );
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setHighlightedIndex((i) => (i > 0 ? i - 1 : -1));
              } else if (e.key === "Enter") {
                e.preventDefault();
                if (highlightedIndex >= 0 && filteredSuggestions[highlightedIndex]) {
                  selectSuggestion(filteredSuggestions[highlightedIndex].name);
                } else {
                  addTag();
                }
              } else if (e.key === "Escape") {
                setEditing(false);
                setInput("");
              }
            }}
            onBlur={(e) => {
              // Delay to allow click on suggestion
              const related = e.relatedTarget as HTMLElement | null;
              if (related && dropdownRef.current?.contains(related)) return;
              setTimeout(() => {
                if (input.trim()) addTag();
                setEditing(false);
              }, 150);
            }}
            placeholder="Add tag..."
            className="h-6 w-32 text-xs"
            autoFocus
            disabled={loading}
          />
          {filteredSuggestions.length > 0 && (
            <div
              ref={dropdownRef}
              className="bg-popover border-border absolute top-full left-0 z-50 mt-1 max-h-40 w-48 overflow-y-auto rounded-md border shadow-md"
            >
              {filteredSuggestions.slice(0, 10).map((s, i) => (
                <button
                  key={s.id}
                  type="button"
                  tabIndex={-1}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    selectSuggestion(s.name);
                  }}
                  className={`text-popover-foreground w-full px-2 py-1 text-left text-xs hover:bg-accent ${
                    i === highlightedIndex ? "bg-accent" : ""
                  }`}
                >
                  {s.name}
                </button>
              ))}
            </div>
          )}
        </div>
      ) : (
        <button
          onClick={() => setEditing(true)}
          className="text-muted-foreground hover:text-foreground"
          disabled={loading}
          title="Add tag"
        >
          <Plus className="h-3.5 w-3.5" />
        </button>
      )}
    </div>
  );
}
