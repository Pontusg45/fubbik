import { useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";
import { X, Plus } from "lucide-react";

interface InlineTagEditorProps {
  tags: Array<{ id: string; name: string }>;
  onUpdate: (tagNames: string[]) => void;
  loading?: boolean;
}

export function InlineTagEditor({ tags, onUpdate, loading }: InlineTagEditorProps) {
  const [editing, setEditing] = useState(false);
  const [input, setInput] = useState("");

  const tagNames = tags.map((t) => t.name);

  const addTag = () => {
    const tag = input.trim().toLowerCase();
    if (tag && !tagNames.includes(tag)) {
      onUpdate([...tagNames, tag]);
    }
    setInput("");
  };

  const removeTag = (name: string) => {
    onUpdate(tagNames.filter((t) => t !== name));
  };

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
        <Input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              addTag();
            }
            if (e.key === "Escape") {
              setEditing(false);
              setInput("");
            }
          }}
          onBlur={() => {
            if (input.trim()) addTag();
            setEditing(false);
          }}
          placeholder="Add tag..."
          className="h-6 w-24 text-xs"
          autoFocus
          disabled={loading}
        />
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
