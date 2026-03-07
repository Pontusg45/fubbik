import mermaid from "mermaid";
import { useEffect, useRef } from "react";
import Markdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import rehypeMermaid from "rehype-mermaid";
import remarkGfm from "remark-gfm";

const remarkPlugins = [remarkGfm];
const rehypePlugins = [
    [rehypeMermaid, { strategy: "pre-mermaid" }],
    rehypeHighlight
] as Parameters<typeof Markdown>[0]["rehypePlugins"];

mermaid.initialize({ startOnLoad: false, theme: "dark" });

export function MarkdownRenderer({ children }: { children: string }) {
    const ref = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (!ref.current) return;
        const mermaidBlocks = ref.current.querySelectorAll("pre.mermaid");
        if (mermaidBlocks.length > 0) {
            mermaid.run({ nodes: mermaidBlocks as NodeListOf<HTMLElement> });
        }
    }, [children]);

    return (
        <div ref={ref}>
            <Markdown remarkPlugins={remarkPlugins} rehypePlugins={rehypePlugins}>
                {children}
            </Markdown>
        </div>
    );
}
