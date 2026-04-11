const WORDS_PER_MINUTE = 200;

export function estimateReadingTime(content: string | null | undefined): { minutes: number; label: string } {
    if (!content) return { minutes: 0, label: "< 1 min" };
    const words = content.trim().split(/\s+/).filter(Boolean).length;
    if (words === 0) return { minutes: 0, label: "< 1 min" };
    const minutes = Math.max(1, Math.round(words / WORDS_PER_MINUTE));
    return { minutes, label: `${minutes} min read` };
}
