import { Tabs, TabsContent, TabsList, TabsTab } from "@/components/ui/tabs";

interface MarkdownEditorProps {
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    rows?: number;
    error?: string;
}

function renderMarkdownPreview(content: string) {
    if (!content.trim()) {
        return <p className="text-muted-foreground text-sm italic">Nothing to preview</p>;
    }

    return (
        <div className="prose prose-invert prose-sm max-w-none">
            {content.split("\n\n").map((block, i) => {
                if (block.startsWith("## ")) {
                    return (
                        <h2 key={i} className="mt-6 mb-2 text-base font-semibold">
                            {block.replace("## ", "")}
                        </h2>
                    );
                }
                if (block.startsWith("- ")) {
                    return (
                        <ul key={i} className="text-muted-foreground list-disc space-y-1 pl-5 text-sm">
                            {block.split("\n").map((line, j) => (
                                <li key={j}>{line.replace(/^- /, "")}</li>
                            ))}
                        </ul>
                    );
                }
                return (
                    <p key={i} className="text-muted-foreground text-sm leading-relaxed">
                        {block}
                    </p>
                );
            })}
        </div>
    );
}

export function MarkdownEditor({ value, onChange, placeholder, rows = 10, error }: MarkdownEditorProps) {
    return (
        <div>
            <label className="mb-1.5 block text-sm font-medium">Content</label>
            <Tabs defaultValue={0}>
                <TabsList>
                    <TabsTab value={0}>Write</TabsTab>
                    <TabsTab value={1}>Preview</TabsTab>
                </TabsList>
                <TabsContent value={0}>
                    <textarea
                        value={value}
                        onChange={e => onChange(e.target.value)}
                        placeholder={placeholder}
                        rows={rows}
                        className="bg-background focus:ring-ring w-full resize-y rounded-md border px-3 py-2 text-sm focus:ring-2 focus:outline-none"
                    />
                </TabsContent>
                <TabsContent value={1}>
                    <div className="min-h-[120px] rounded-md border px-3 py-2">
                        {renderMarkdownPreview(value)}
                    </div>
                </TabsContent>
            </Tabs>
            {error && <p className="text-destructive mt-1 text-xs">{error}</p>}
        </div>
    );
}
