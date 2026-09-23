import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Input } from "@/components/ui/input";

export function FieldFixtures() {
    const [type, setType] = useState("note");
    const [input, setInput] = useState("");
    const [tags, setTags] = useState<string[]>([]);
    return (
        <section aria-label="Native fields">
            <label htmlFor="native-type">Type</label>
            <select id="native-type" value={type} onChange={event => setType(event.target.value)}>
                <option value="note">Note</option>
                <option value="document">Document</option>
            </select>
            <output aria-label="Native type">{type}</output>
            <label htmlFor="fixture-tags">Tags</label>
            <div>
                {tags.map(tag => (
                    <Badge key={tag} onClick={() => setTags(tags.filter(value => value !== tag))}>
                        {tag} ×
                    </Badge>
                ))}
            </div>
            <Input
                id="fixture-tags"
                value={input}
                onChange={event => setInput(event.target.value)}
                onKeyDown={event => {
                    if (event.key !== "Enter") return;
                    event.preventDefault();
                    const tag = input.trim().toLowerCase();
                    if (tag && !tags.includes(tag)) setTags([...tags, tag]);
                    setInput("");
                }}
            />
        </section>
    );
}
