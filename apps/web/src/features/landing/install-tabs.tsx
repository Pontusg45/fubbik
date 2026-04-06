import { Check, Copy } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";

const tabs = [
    {
        label: "Local",
        lines: [
            { code: "git clone https://github.com/Pontusg45/fubbik.git" },
            { code: "cd fubbik" },
            { code: "pnpm install" },
            { code: "pnpm seed", comment: "# sample data" },
            { code: "pnpm dev", comment: "# localhost:3001" },
        ],
    },
    {
        label: "Docker",
        badge: "soon",
        lines: [
            { code: "git clone https://github.com/Pontusg45/fubbik.git" },
            { code: "cd fubbik" },
            { code: "docker compose up" },
        ],
    },
    {
        label: "npm",
        badge: "soon",
        lines: [
            { code: "npx create-fubbik my-knowledge-base" },
            { code: "cd my-knowledge-base" },
            { code: "pnpm dev" },
        ],
    },
] as const;

export function InstallTabs() {
    const [activeTab, setActiveTab] = useState(0);
    const [copied, setCopied] = useState(false);

    const currentTab = tabs[activeTab];

    function handleCopy() {
        const text = currentTab.lines
            .map((line) =>
                "comment" in line && line.comment
                    ? `${line.code}    ${line.comment}`
                    : line.code
            )
            .join("\n");
        navigator.clipboard.writeText(text);
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
    }

    return (
        <div className="max-w-lg mx-auto">
            {/* Tab buttons */}
            <div className="flex rounded-t-lg border border-b-0 bg-muted/30">
                {tabs.map((tab, index) => (
                    <button
                        key={tab.label}
                        type="button"
                        onClick={() => setActiveTab(index)}
                        className={`flex items-center gap-1.5 px-4 py-2 text-sm font-medium transition-colors ${
                            activeTab === index
                                ? "bg-background text-foreground shadow-sm"
                                : "text-muted-foreground hover:text-foreground"
                        } ${index === 0 ? "rounded-tl-lg" : ""}`}
                    >
                        {tab.label}
                        {"badge" in tab && tab.badge && (
                            <Badge variant="outline" size="sm">
                                {tab.badge}
                            </Badge>
                        )}
                    </button>
                ))}
            </div>

            {/* Code block */}
            <div className="relative rounded-b-lg border bg-[#0d1117] p-4">
                <button
                    type="button"
                    onClick={handleCopy}
                    className="absolute top-3 right-3 p-1.5 rounded-md text-white/40 hover:text-white/70 transition-colors"
                    aria-label="Copy to clipboard"
                >
                    {copied ? (
                        <Check className="size-4 text-emerald-400" />
                    ) : (
                        <Copy className="size-4" />
                    )}
                </button>

                <pre className="font-mono text-[13px] leading-relaxed text-white/70">
                    {currentTab.lines.map((line, index) => (
                        <div key={index}>
                            <span>{line.code}</span>
                            {"comment" in line && line.comment && (
                                <span className="text-white/30">
                                    {"    "}
                                    {line.comment}
                                </span>
                            )}
                        </div>
                    ))}
                </pre>
            </div>
        </div>
    );
}
