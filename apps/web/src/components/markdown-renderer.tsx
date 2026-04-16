import { useEffect, useId, useState } from "react";
import Markdown, { type Components } from "react-markdown";
import rehypeRaw from "rehype-raw";
import remarkGfm from "remark-gfm";

let mermaidReady: typeof import("mermaid").default | null = null;

if (typeof window !== "undefined") {
    import("mermaid").then(m => {
        mermaidReady = m.default;
        mermaidReady.initialize({ startOnLoad: false, theme: "dark" });
    });
}

const remarkPlugins = [remarkGfm];
const rehypePlugins = [rehypeRaw];

/* ─── Mermaid block ─── */

function MermaidBlock({ children }: { children: string }) {
    const id = useId().replace(/:/g, "-");
    const [svg, setSvg] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        let cancelled = false;

        function tryRender() {
            if (!mermaidReady) {
                // Mermaid still loading — retry shortly
                setTimeout(tryRender, 100);
                return;
            }
            mermaidReady
                .render(`mermaid-${id}`, children.trim())
                .then(({ svg: rendered }) => {
                    if (!cancelled) setSvg(rendered);
                })
                .catch(err => {
                    if (!cancelled) setError(String(err));
                });
        }
        tryRender();

        return () => { cancelled = true; };
    }, [children, id]);

    if (error) {
        return (
            <pre className="overflow-x-auto rounded-lg bg-red-950/30 border border-red-500/20 p-4 text-sm text-red-400">
                <code>{children}</code>
            </pre>
        );
    }

    if (!svg) {
        return (
            <div className="flex items-center justify-center rounded-lg border border-border/40 bg-muted/20 p-8 text-sm text-muted-foreground">
                Rendering diagram...
            </div>
        );
    }

    return (
        <div
            className="my-4 flex justify-center overflow-x-auto [&_svg]:max-w-full"
            dangerouslySetInnerHTML={{ __html: svg }}
        />
    );
}

/* ─── Syntax-highlighted code block ─── */

function CodeBlock({ className, children }: { className?: string; children: string }) {
    const [html, setHtml] = useState<string | null>(null);
    const code = String(children).replace(/\n$/, "");
    const lang = className?.replace(/^language-/, "") ?? "";

    useEffect(() => {
        if (lang === "mermaid") return;
        let cancelled = false;

        import("shiki").then(({ codeToHtml }) =>
            codeToHtml(code, {
                lang: lang || "text",
                themes: { light: "github-light", dark: "github-dark-dimmed" },
                defaultColor: false,
            })
        )
            .then(result => {
                if (!cancelled) setHtml(result);
            })
            .catch(() => {
                // Unknown language — fall back to plain
                if (!cancelled) setHtml(null);
            });

        return () => { cancelled = true; };
    }, [code, lang]);

    if (lang === "mermaid") {
        return <MermaidBlock>{code}</MermaidBlock>;
    }

    if (html) {
        return (
            <div
                className="my-3 overflow-x-auto rounded-lg border border-border/30 bg-[#f6f8fa] text-sm dark:bg-[#22272e] [&_pre]:!p-4 [&_pre]:!rounded-lg [&_.shiki]:!bg-transparent"
                dangerouslySetInnerHTML={{ __html: html }}
            />
        );
    }

    // Fallback while shiki is loading or for unknown langs
    return (
        <pre className="my-3 overflow-x-auto rounded-lg border border-border/30 bg-[#f6f8fa] p-4 text-sm dark:bg-[#22272e]">
            <code>{code}</code>
        </pre>
    );
}

/* ─── Component overrides ─── */

const components: Components = {
    pre({ children }) {
        // react-markdown wraps <code> in <pre> — unwrap it so CodeBlock handles rendering
        return <>{children}</>;
    },
    code({ className, children, ...props }) {
        const isInline = !className && typeof children === "string" && !children.includes("\n");
        if (isInline) {
            return (
                <code className="rounded bg-muted px-1.5 py-0.5 text-sm font-mono" {...props}>
                    {children}
                </code>
            );
        }
        return <CodeBlock className={className}>{String(children)}</CodeBlock>;
    },
    table({ children }) {
        return (
            <div className="my-4 overflow-x-auto rounded-lg border border-border">
                <table className="w-full text-sm">{children}</table>
            </div>
        );
    },
    thead({ children }) {
        return <thead className="border-b border-border bg-muted/50">{children}</thead>;
    },
    th({ children }) {
        return <th className="px-4 py-2 text-left text-xs font-semibold text-muted-foreground">{children}</th>;
    },
    td({ children }) {
        return <td className="border-t border-border/50 px-4 py-2">{children}</td>;
    },
    blockquote({ children }) {
        return (
            <blockquote className="my-3 border-l-2 border-primary/40 pl-4 text-muted-foreground italic">
                {children}
            </blockquote>
        );
    },
    a({ href, children }) {
        return (
            <a
                href={href}
                className="text-primary underline underline-offset-2 hover:text-primary/80"
                target={href?.startsWith("http") ? "_blank" : undefined}
                rel={href?.startsWith("http") ? "noopener noreferrer" : undefined}
            >
                {children}
            </a>
        );
    },
    hr() {
        return <hr className="my-6 border-border/50" />;
    },
    img({ src, alt }) {
        return (
            <img
                src={src}
                alt={alt ?? ""}
                className="my-4 max-w-full rounded-lg border border-border/30"
                loading="lazy"
            />
        );
    },
};

/* ─── Renderer ─── */

export function MarkdownRenderer({ children }: { children: string }) {
    return (
        <Markdown
            remarkPlugins={remarkPlugins}
            rehypePlugins={rehypePlugins}
            components={components}
        >
            {children}
        </Markdown>
    );
}
