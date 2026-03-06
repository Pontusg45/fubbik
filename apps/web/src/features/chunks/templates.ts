export interface ChunkTemplate {
    name: string;
    description: string;
    type: string;
    tags: string[];
    content: string;
}

export const chunkTemplates: ChunkTemplate[] = [
    {
        name: "Meeting Notes",
        description: "Structured meeting notes with attendees and action items",
        type: "note",
        tags: ["meeting"],
        content: `## Attendees\n\n- \n\n## Agenda\n\n- \n\n## Discussion\n\n\n\n## Action Items\n\n- [ ] `
    },
    {
        name: "Decision Record",
        description: "Document a technical or product decision",
        type: "document",
        tags: ["decision"],
        content: `## Context\n\nWhat is the issue that we're seeing that is motivating this decision?\n\n## Decision\n\nWhat is the change that we're proposing and/or doing?\n\n## Consequences\n\nWhat becomes easier or harder because of this change?`
    },
    {
        name: "API Reference",
        description: "Document an API endpoint or service",
        type: "reference",
        tags: ["api"],
        content: `## Endpoint\n\n\`METHOD /path\`\n\n## Request\n\n\n\n## Response\n\n\n\n## Examples\n\n`
    },
    {
        name: "Checklist",
        description: "A reusable checklist",
        type: "checklist",
        tags: ["checklist"],
        content: `- [ ] \n- [ ] \n- [ ] `
    },
    {
        name: "Schema",
        description: "Document a data model or schema",
        type: "schema",
        tags: ["schema"],
        content: `## Fields\n\n- \`id\` — \n- \`name\` — \n\n## Relations\n\n\n\n## Constraints\n\n`
    }
];
