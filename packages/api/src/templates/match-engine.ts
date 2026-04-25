import type {
    MatchRules,
    ParsedHeading,
    TemplateWithRules,
    TemplateMatch,
} from "./types";

export function matchHeading(
    pattern: string,
    heading: ParsedHeading,
    mode: "exact" | "prefix" | "contains",
): boolean {
    const p = pattern.toLowerCase();
    const h = heading.text.toLowerCase();
    switch (mode) {
        case "exact":
            return h === p;
        case "prefix":
            return h.startsWith(p);
        case "contains":
            return h.includes(p);
    }
}

export function scoreTemplate(
    rules: MatchRules,
    headings: ParsedHeading[],
    frontmatter: Record<string, unknown>,
): number {
    let score = 0;

    for (const rule of rules.headings) {
        const matched = headings.some((h) => {
            if (rule.level !== undefined && h.level !== rule.level) return false;
            return rule.patterns.some((p) => matchHeading(p, h, rule.match));
        });
        if (rule.required && !matched) return 0;
        if (matched) score += rule.required ? 1 : 0.5;
    }

    for (const rule of rules.frontmatter) {
        const value = frontmatter[rule.key];
        let matched = false;
        switch (rule.match) {
            case "exists":
                matched = value !== undefined;
                break;
            case "exact":
                matched = String(value) === rule.value;
                break;
            case "oneOf":
                matched = rule.values?.includes(String(value)) ?? false;
                break;
        }
        if (matched) score += 1;
    }

    return score >= rules.minScore ? score : 0;
}

export function matchTemplates(
    doc: { headings: ParsedHeading[]; frontmatter: Record<string, unknown> },
    templates: TemplateWithRules[],
): TemplateMatch | null {
    if (templates.length === 0) return null;

    const scored = templates
        .map((t) => ({
            template: t,
            score: scoreTemplate(t.matchRules, doc.headings, doc.frontmatter),
        }))
        .filter((s) => s.score > 0)
        .sort((a, b) => {
            if (b.score !== a.score) return b.score - a.score;
            if (b.template.priority !== a.template.priority)
                return b.template.priority - a.template.priority;
            const aReq = a.template.matchRules.headings.filter((h) => h.required).length;
            const bReq = b.template.matchRules.headings.filter((h) => h.required).length;
            return bReq - aReq;
        });

    if (scored.length === 0) return null;

    const best = scored[0]!;
    return {
        templateId: best.template.id,
        templateName: best.template.name,
        score: best.score,
        type: best.template.type,
        tags: best.template.tags ?? [],
        extractedFields: {},
    };
}
