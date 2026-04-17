/**
 * Core module: builtin vocabulary catalogs (chunk_type + connection_relation).
 *
 * These are idempotent via ON CONFLICT DO UPDATE — safe to run against any
 * database state, always produces the canonical builtin set. No reset needed;
 * other modules depend on these slugs being present.
 */

import { eq } from "drizzle-orm";

import { chunkType } from "../../schema/chunk-type";
import { connectionRelation } from "../../schema/connection-relation";
import type { SeedContext } from "../context";

const BUILTIN_CHUNK_TYPES = [
    { id: "note",       label: "Note",        description: "A free-form note or observation",                    icon: "StickyNote", color: "#94a3b8", displayOrder: 10, examples: ["Quick thought", "TODO", "Question"] },
    { id: "document",   label: "Document",    description: "Longer-form written content",                        icon: "FileText",   color: "#3b82f6", displayOrder: 20, examples: ["Spec", "RFC", "Meeting notes"] },
    { id: "guide",      label: "Guide",       description: "Step-by-step instructions or tutorial",              icon: "BookOpen",   color: "#6366f1", displayOrder: 30, examples: ["Onboarding", "How-to"] },
    { id: "reference",  label: "Reference",   description: "Lookup material — APIs, glossary, canonical links", icon: "Compass",    color: "#14b8a6", displayOrder: 40, examples: ["API shape", "Glossary entry"] },
    { id: "schema",     label: "Schema",      description: "Data model or structural definition",                icon: "Database",   color: "#f59e0b", displayOrder: 50, examples: ["Table schema", "Event payload"] },
    { id: "checklist",  label: "Checklist",   description: "Ordered items to verify or complete",                icon: "CheckSquare",color: "#84cc16", displayOrder: 60, examples: ["Launch checklist", "Review items"] },
    { id: "convention", label: "Convention",  description: "A rule the team agrees to follow",                   icon: "Scale",      color: "#ec4899", displayOrder: 70, examples: ["Naming pattern", "Code style"] }
] as const;

const BUILTIN_RELATIONS = [
    { id: "related_to",     label: "Related to",     description: "General relationship — the weakest link",                       arrowStyle: "dashed", direction: "bidirectional", color: "#94a3b8", displayOrder: 10 },
    { id: "part_of",        label: "Part of",        description: "Source is a component of target",                                arrowStyle: "solid",  direction: "forward",        color: "#3b82f6", displayOrder: 20 },
    { id: "contains",       label: "Contains",       description: "Source is a container holding target (inverse of part_of)",      arrowStyle: "solid",  direction: "forward",        color: "#3b82f6", displayOrder: 21 },
    { id: "depends_on",     label: "Depends on",     description: "Source requires target to work",                                 arrowStyle: "solid",  direction: "forward",        color: "#f59e0b", displayOrder: 30 },
    { id: "required_by",    label: "Required by",    description: "Target depends on source (inverse of depends_on)",               arrowStyle: "solid",  direction: "forward",        color: "#f59e0b", displayOrder: 31 },
    { id: "extends",        label: "Extends",        description: "Source specializes or builds upon target",                       arrowStyle: "solid",  direction: "forward",        color: "#6366f1", displayOrder: 40 },
    { id: "extended_by",    label: "Extended by",    description: "Target extends source (inverse of extends)",                     arrowStyle: "solid",  direction: "forward",        color: "#6366f1", displayOrder: 41 },
    { id: "references",     label: "References",     description: "Source mentions or cites target",                                arrowStyle: "dotted", direction: "forward",        color: "#14b8a6", displayOrder: 50 },
    { id: "referenced_by",  label: "Referenced by",  description: "Target references source (inverse of references)",               arrowStyle: "dotted", direction: "forward",        color: "#14b8a6", displayOrder: 51 },
    { id: "supports",       label: "Supports",       description: "Source provides evidence for target",                            arrowStyle: "solid",  direction: "forward",        color: "#22c55e", displayOrder: 60 },
    { id: "supported_by",   label: "Supported by",   description: "Target supports source (inverse of supports)",                   arrowStyle: "solid",  direction: "forward",        color: "#22c55e", displayOrder: 61 },
    { id: "contradicts",    label: "Contradicts",    description: "Source disagrees with target",                                   arrowStyle: "solid",  direction: "bidirectional", color: "#ef4444", displayOrder: 70 },
    { id: "alternative_to", label: "Alternative to", description: "Source and target are competing approaches",                     arrowStyle: "dashed", direction: "bidirectional", color: "#a855f7", displayOrder: 80 }
] as const;

const INVERSE_PAIRS: Array<[string, string]> = [
    ["depends_on", "required_by"],
    ["part_of", "contains"],
    ["extends", "extended_by"],
    ["references", "referenced_by"],
    ["supports", "supported_by"]
];

export async function seed(ctx: SeedContext): Promise<void> {
    for (const t of BUILTIN_CHUNK_TYPES) {
        await ctx.db
            .insert(chunkType)
            .values({ ...t, builtIn: true, examples: [...t.examples] })
            .onConflictDoUpdate({
                target: chunkType.id,
                set: {
                    label: t.label,
                    description: t.description,
                    icon: t.icon,
                    color: t.color,
                    displayOrder: t.displayOrder,
                    examples: [...t.examples],
                    builtIn: true
                }
            });
    }
    ctx.counters["chunk_types"] = BUILTIN_CHUNK_TYPES.length;

    for (const r of BUILTIN_RELATIONS) {
        await ctx.db
            .insert(connectionRelation)
            .values({ ...r, builtIn: true })
            .onConflictDoUpdate({
                target: connectionRelation.id,
                set: {
                    label: r.label,
                    description: r.description,
                    arrowStyle: r.arrowStyle,
                    direction: r.direction,
                    color: r.color,
                    displayOrder: r.displayOrder,
                    builtIn: true
                }
            });
    }
    for (const [a, b] of INVERSE_PAIRS) {
        await ctx.db.update(connectionRelation).set({ inverseOfId: b }).where(eq(connectionRelation.id, a));
        await ctx.db.update(connectionRelation).set({ inverseOfId: a }).where(eq(connectionRelation.id, b));
    }
    ctx.counters["connection_relations"] = BUILTIN_RELATIONS.length;
}

// No reset — catalogs are builtins and outlive individual runs.
