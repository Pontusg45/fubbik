import { createContext, memo, useContext, useEffect, useId, useMemo, useState } from "react";
import { Link } from "@tanstack/react-router";
import Markdown, { type Components } from "react-markdown";
import rehypeRaw from "rehype-raw";
import remarkGfm from "remark-gfm";
import { matchInCode, matchVocabularyInText, useSmartLinks } from "./smart-link-provider";
import { VocabularyPopover } from "./vocabulary-popover";

const mermaidPromise = typeof window !== "undefined"
    ? import("mermaid").then(m => {
        m.default.initialize({ startOnLoad: false, theme: "dark" });
        return m.default;
    })
    : null;

const remarkPlugins = [remarkGfm];
const rehypePlugins = [rehypeRaw];

/* ─── Mermaid block ─── */

function MermaidBlock({ children }: { children: string }) {
    const id = useId().replace(/:/g, "-");
    const [svg, setSvg] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        let cancelled = false;
        if (!mermaidPromise) return;

        mermaidPromise
            .then(mermaid => mermaid.render(`mermaid-${id}`, children.trim()))
            .then(({ svg: rendered }) => {
                if (!cancelled) setSvg(rendered);
            })
            .catch(err => {
                if (!cancelled) setError(String(err));
            });

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

/* ─── Copy button ─── */

function CopyButton({ code }: { code: string }) {
    const [copied, setCopied] = useState(false);

    const handleCopy = () => {
        navigator.clipboard.writeText(code).then(() => {
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        });
    };

    return (
        <button
            onClick={handleCopy}
            className="rounded p-0.5 text-muted-foreground/60 opacity-0 transition-all hover:text-foreground group-hover:opacity-100"
            aria-label="Copy code"
        >
            {copied ? (
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <polyline points="20 6 9 17 4 12" />
                </svg>
            ) : (
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
                </svg>
            )}
        </button>
    );
}

/* ─── Language label mapping ─── */

const LANG_LABELS: Record<string, string> = {
    ts: "TypeScript", typescript: "TypeScript",
    js: "JavaScript", javascript: "JavaScript",
    tsx: "TSX", jsx: "JSX",
    py: "Python", python: "Python",
    rb: "Ruby", ruby: "Ruby",
    rs: "Rust", rust: "Rust",
    go: "Go", sh: "Shell", bash: "Bash", zsh: "Shell",
    sql: "SQL", json: "JSON", yaml: "YAML", yml: "YAML",
    html: "HTML", css: "CSS", scss: "SCSS",
    md: "Markdown", markdown: "Markdown",
    toml: "TOML", xml: "XML", graphql: "GraphQL",
    dockerfile: "Dockerfile", docker: "Docker",
    text: "Plain Text",
};

function langLabel(lang: string): string | null {
    if (!lang) return null;
    return LANG_LABELS[lang.toLowerCase()] ?? lang;
}

/* ─── Syntax-highlighted code block ─── */

function CodeBlock({ className, children }: { className?: string; children: string }) {
    const [html, setHtml] = useState<string | null>(null);
    const code = String(children).replace(/\n$/, "");
    const lang = className?.replace(/^language-/, "") ?? "";
    const label = langLabel(lang);
    const lineCount = code.split("\n").length;

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
                if (!cancelled) setHtml(null);
            });

        return () => { cancelled = true; };
    }, [code, lang]);

    if (lang === "mermaid") {
        return <MermaidBlock>{code}</MermaidBlock>;
    }

    const header = label ? (
        <div className="flex items-center justify-between border-b border-border/20 bg-muted/30 px-4 py-1.5">
            <span className="text-[10px] font-medium text-muted-foreground">{label}</span>
            <div className="flex items-center gap-2">
                <span className="text-[10px] text-muted-foreground/60">{lineCount} lines</span>
                <CopyButton code={code} />
            </div>
        </div>
    ) : null;

    if (html) {
        return (
            <div className="group relative my-3 overflow-hidden rounded-lg border border-border/30 bg-[#f6f8fa] dark:bg-[#22272e]">
                {header}
                <div
                    className="overflow-x-auto font-mono text-sm [&_pre]:!p-4 [&_pre]:!m-0 [&_pre]:!rounded-none [&_.shiki]:!bg-transparent"
                    dangerouslySetInnerHTML={{ __html: html }}
                />
            </div>
        );
    }

    return (
        <div className="group relative my-3 overflow-hidden rounded-lg border border-border/30 bg-[#f6f8fa] dark:bg-[#22272e]">
            {header}
            <pre className="overflow-x-auto p-4 font-mono text-sm">
                <code className="text-muted-foreground">{code}</code>
            </pre>
        </div>
    );
}

const ExcludeChunkContext = createContext<string | undefined>(undefined);

/* ─── Smart inline code ─── */

function SmartCode({ children, className, ...props }: {
    children: string;
    className?: string;
    [key: string]: unknown;
}) {
    const { chunkIndex, fileRefIndex, vocabIndex } = useSmartLinks();
    const excludeChunkId = useContext(ExcludeChunkContext);
    const text = String(children);
    const isInline = !className && !text.includes("\n");

    if (!isInline) {
        return <CodeBlock className={className}>{text}</CodeBlock>;
    }

    const match = matchInCode(text, chunkIndex, fileRefIndex, vocabIndex, excludeChunkId);

    if (match?.type === "chunk") {
        return (
            <Link
                to="/chunks/$chunkId"
                params={{ chunkId: match.id }}
                className="rounded bg-primary/10 px-1.5 py-0.5 text-sm font-mono text-primary hover:bg-primary/20 transition-colors"
            >
                {children}
            </Link>
        );
    }

    if (match?.type === "fileRef") {
        return (
            <Link
                to="/chunks/$chunkId"
                params={{ chunkId: match.chunkId }}
                className="rounded bg-primary/10 px-1.5 py-0.5 text-sm font-mono text-primary hover:bg-primary/20 transition-colors"
                title={match.path}
            >
                {children}
            </Link>
        );
    }

    if (match?.type === "vocabulary") {
        return (
            <VocabularyPopover word={match.word} definition={match.definition} category={match.category} expects={match.expects}>
                <code className="rounded bg-muted px-1.5 py-0.5 text-sm font-mono" {...props}>
                    {children}
                </code>
            </VocabularyPopover>
        );
    }

    return (
        <code className="rounded bg-muted/80 border border-border/40 px-1.5 py-0.5 text-[0.85em] font-mono" {...props}>
            {children}
        </code>
    );
}

/* ─── Smart text (vocabulary matching in prose) ─── */

const SmartText = memo(function SmartText({ children }: { children: string }) {
    const { vocabIndex, vocabPattern } = useSmartLinks();
    const matches = useMemo(
        () => matchVocabularyInText(children, vocabIndex, vocabPattern),
        [children, vocabIndex, vocabPattern]
    );

    if (matches.length === 0) return <>{children}</>;

    const parts: React.ReactNode[] = [];
    let lastEnd = 0;

    for (const match of matches) {
        if (match.start > lastEnd) {
            parts.push(children.slice(lastEnd, match.start));
        }
        parts.push(
            <VocabularyPopover
                key={match.start}
                word={match.word}
                definition={match.definition}
                category={match.category}
                expects={match.expects}
            >
                {children.slice(match.start, match.end)}
            </VocabularyPopover>
        );
        lastEnd = match.end;
    }

    if (lastEnd < children.length) {
        parts.push(children.slice(lastEnd));
    }

    return <>{parts}</>;
});

/* ─── Smart paragraph (wraps text children with vocab matching) ─── */

const SmartParagraph = memo(function SmartParagraph({ children }: { children: React.ReactNode }) {
    return <p>{processChildren(children)}</p>;
});

const SmartListItem = memo(function SmartListItem({ children }: { children: React.ReactNode }) {
    return <li>{processChildren(children)}</li>;
});

function processChildren(children: React.ReactNode): React.ReactNode {
    if (typeof children === "string") {
        return <SmartText>{children}</SmartText>;
    }
    if (Array.isArray(children)) {
        return children.map((child, i) => {
            if (typeof child === "string") {
                return <SmartText key={i}>{child}</SmartText>;
            }
            return child;
        });
    }
    return children;
}

/* ─── Table of contents helpers ─── */

function slugify(text: string): string {
    return text
        .toLowerCase()
        .replace(/[^\w\s-]/g, "")
        .replace(/\s+/g, "-")
        .replace(/-+/g, "-")
        .trim();
}

interface TocEntry {
    level: number;
    text: string;
    slug: string;
}

function extractToc(markdown: string): TocEntry[] {
    const entries: TocEntry[] = [];
    const regex = /^(#{1,6})\s+(.+)$/gm;
    let match: RegExpExecArray | null;
    while ((match = regex.exec(markdown)) !== null) {
        entries.push({
            level: match[1]!.length,
            text: match[2]!.trim(),
            slug: slugify(match[2]!.trim()),
        });
    }
    return entries;
}

function TableOfContents({ entries }: { entries: TocEntry[] }) {
    const minLevel = Math.min(...entries.map(e => e.level));

    return (
        <nav className="mb-6 rounded-lg border border-border/50 bg-muted/20 px-4 py-3">
            <p className="mb-2 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                Table of Contents
            </p>
            <ul className="space-y-1 text-sm">
                {entries.map((entry, i) => (
                    <li key={i} style={{ marginLeft: `${(entry.level - minLevel) * 16}px` }}>
                        <a
                            href={`#${entry.slug}`}
                            className="text-muted-foreground hover:text-foreground transition-colors"
                        >
                            {entry.text}
                        </a>
                    </li>
                ))}
            </ul>
        </nav>
    );
}

/* ─── Component overrides ─── */

const components: Components = {
    pre({ children }) {
        // react-markdown wraps <code> in <pre> — unwrap it so CodeBlock handles rendering
        return <>{children}</>;
    },
    code({ className, children, ...props }) {
        const text = String(children);
        const isInline = !className && !text.includes("\n");
        if (isInline) {
            return <SmartCode className={className} {...props}>{text}</SmartCode>;
        }
        return <CodeBlock className={className}>{text}</CodeBlock>;
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
    p({ children }) {
        return <SmartParagraph>{children}</SmartParagraph>;
    },
    li({ children }) {
        return <SmartListItem>{children}</SmartListItem>;
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
    h1({ children }) {
        const text = typeof children === "string" ? children : String(children);
        return <h1 id={slugify(text)}>{children}</h1>;
    },
    h2({ children }) {
        const text = typeof children === "string" ? children : String(children);
        return <h2 id={slugify(text)}>{children}</h2>;
    },
    h3({ children }) {
        const text = typeof children === "string" ? children : String(children);
        return <h3 id={slugify(text)}>{children}</h3>;
    },
    h4({ children }) {
        const text = typeof children === "string" ? children : String(children);
        return <h4 id={slugify(text)}>{children}</h4>;
    },
    h5({ children }) {
        const text = typeof children === "string" ? children : String(children);
        return <h5 id={slugify(text)}>{children}</h5>;
    },
    h6({ children }) {
        const text = typeof children === "string" ? children : String(children);
        return <h6 id={slugify(text)}>{children}</h6>;
    },
};

/* ─── Renderer ─── */

export function MarkdownRenderer({ children, excludeChunkId }: { children: string; excludeChunkId?: string }) {
    const tocEntries = useMemo(() => extractToc(children), [children]);

    return (
        <ExcludeChunkContext.Provider value={excludeChunkId}>
            {tocEntries.length >= 3 && <TableOfContents entries={tocEntries} />}
            <Markdown
                remarkPlugins={remarkPlugins}
                rehypePlugins={rehypePlugins}
                components={components}
            >
                {children}
            </Markdown>
        </ExcludeChunkContext.Provider>
    );
}
