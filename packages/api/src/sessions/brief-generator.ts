interface BriefInput {
    session: { title: string; createdAt: Date; completedAt?: Date | null };
    chunkRefs: Array<{ chunkId: string; chunkTitle: string; reason: string }>;
    assumptions: Array<{ id: string; description: string }>;
    requirementRefs: Array<{
        requirementId: string;
        requirementTitle: string;
        requirementStatus: string;
        totalSteps: number;
        stepsAddressed: number[];
    }>;
    allRequirements: Array<{ id: string; title: string; status: string; steps: unknown[] }>;
    allConventions: Array<{ id: string; title: string }>;
}

export function generateReviewBrief(input: BriefInput): string {
    const lines: string[] = [];

    // Summary Stats
    lines.push("## Summary");
    lines.push("");
    const addressed = input.requirementRefs.length;
    const total = input.allRequirements.length;
    lines.push(`- **Requirements addressed:** ${addressed} / ${total}`);
    lines.push(`- **Chunks referenced:** ${input.chunkRefs.length}`);
    lines.push(`- **Assumptions made:** ${input.assumptions.length}`);
    if (input.session.completedAt && input.session.createdAt) {
        const durationMs =
            new Date(input.session.completedAt).getTime() - new Date(input.session.createdAt).getTime();
        const minutes = Math.round(durationMs / 60000);
        lines.push(`- **Duration:** ${minutes} min`);
    }
    lines.push("");

    // Requirements Addressed
    lines.push("## Requirements Addressed");
    lines.push("");
    if (input.requirementRefs.length === 0) {
        lines.push("_No requirements were addressed._");
    } else {
        for (const ref of input.requirementRefs) {
            const stepsText =
                ref.stepsAddressed.length > 0
                    ? `${ref.stepsAddressed.length}/${ref.totalSteps} steps`
                    : `${ref.totalSteps} steps`;
            const partial =
                ref.stepsAddressed.length > 0 && ref.stepsAddressed.length < ref.totalSteps ? " ⚠️ partial" : "";
            lines.push(`- **${ref.requirementTitle}** [${ref.requirementStatus}] — ${stepsText}${partial}`);
        }
    }
    lines.push("");

    // Requirements Not Addressed
    const addressedIds = new Set(input.requirementRefs.map(r => r.requirementId));
    const notAddressed = input.allRequirements.filter(r => !addressedIds.has(r.id));
    if (notAddressed.length > 0) {
        lines.push("## Requirements Not Addressed");
        lines.push("");
        for (const req of notAddressed) {
            lines.push(`- ${req.title} [${req.status}]`);
        }
        lines.push("");
    }

    // Conventions Applied
    lines.push("## Conventions Applied");
    lines.push("");
    if (input.chunkRefs.length === 0) {
        lines.push("_No conventions were referenced._");
    } else {
        for (const ref of input.chunkRefs) {
            lines.push(`- **${ref.chunkTitle}** — ${ref.reason}`);
        }
    }
    lines.push("");

    // Conventions Not Checked
    const refChunkIds = new Set(input.chunkRefs.map(r => r.chunkId));
    const notChecked = input.allConventions.filter(c => !refChunkIds.has(c.id));
    if (notChecked.length > 0) {
        lines.push("## Conventions Not Checked");
        lines.push("");
        for (const conv of notChecked) {
            lines.push(`- ${conv.title}`);
        }
        lines.push("");
    }

    // Assumptions
    lines.push("## Assumptions Made");
    lines.push("");
    if (input.assumptions.length === 0) {
        lines.push("_No assumptions were made._");
    } else {
        for (const a of input.assumptions) {
            lines.push(`- [ ] ${a.description}`);
        }
    }
    lines.push("");

    return lines.join("\n");
}
