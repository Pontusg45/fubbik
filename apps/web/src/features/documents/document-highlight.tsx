/* ─── Search highlight helper ─── */

export function highlightMatches(text: string, query: string): React.ReactNode[] {
    if (!query) return [text];
    const regex = new RegExp(`(${query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")})`, "gi");
    const parts = text.split(regex);
    return parts.map((part, i) =>
        regex.test(part) ? (
            <mark key={i} className="rounded bg-yellow-200 px-0.5 dark:bg-yellow-800">
                {part}
            </mark>
        ) : (
            part
        )
    );
}
