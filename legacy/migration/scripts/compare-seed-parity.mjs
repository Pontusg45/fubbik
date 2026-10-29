#!/usr/bin/env node
import { execFileSync } from "node:child_process";

const legacy = process.env.LEGACY_DATABASE_URL;
const native = process.env.RUST_DATABASE_URL;
if (!legacy || !native) {
    console.error("Set LEGACY_DATABASE_URL and RUST_DATABASE_URL to separately seeded databases.");
    process.exit(2);
}

const tables = [
    "chunk_type", "connection_relation", "chunk_template", "space", "space_code_metadata",
    "tag_type", "tag", "chunk", "chunk_tag", "chunk_space", "chunk_connection",
    "chunk_applies_to", "chunk_file_ref", "use_case", "requirement", "requirement_chunk",
    "plan", "plan_task", "plan_analyze_item", "plan_requirement", "agent_run",
    "plan_task_claim", "coordination_entry", "document", "vocabulary_entry",
    "workspace", "workspace_space", "collection", "behavior_matrix", "behavior_dimension",
    "behavior_rule", "behavior_cell", "behavior_cell_requirement"
];

function query(url, sql) {
    return JSON.parse(execFileSync("psql", [url, "-X", "-A", "-t", "-v", "ON_ERROR_STOP=1", "-c", sql], {
        encoding: "utf8"
    }).trim());
}

const countsSql = `SELECT jsonb_build_object(${tables.map(table => `'${table}', (SELECT count(*) FROM ${table})`).join(", ")})`;
const relations = {
    connections: `SELECT coalesce(jsonb_agg(jsonb_build_array(source.title, edge.relation, target.title) ORDER BY source.title, edge.relation, target.title), '[]'::jsonb)
        FROM chunk_connection edge JOIN chunk source ON source.id = edge.source_id JOIN chunk target ON target.id = edge.target_id`,
    chunk_tags: `SELECT coalesce(jsonb_agg(jsonb_build_array(chunk.title, tag.name) ORDER BY chunk.title, tag.name), '[]'::jsonb)
        FROM chunk_tag link JOIN chunk ON chunk.id = link.chunk_id JOIN tag ON tag.id = link.tag_id`,
    requirement_chunks: `SELECT coalesce(jsonb_agg(jsonb_build_array(requirement.title, chunk.title) ORDER BY requirement.title, chunk.title), '[]'::jsonb)
        FROM requirement_chunk link JOIN requirement ON requirement.id = link.requirement_id JOIN chunk ON chunk.id = link.chunk_id`,
    workspace_spaces: `SELECT coalesce(jsonb_agg(jsonb_build_array(workspace.name, space.name) ORDER BY workspace.name, space.name), '[]'::jsonb)
        FROM workspace_space link JOIN workspace ON workspace.id = link.workspace_id JOIN space ON space.id = link.space_id`
};

let mismatches = 0;
const legacyCounts = query(legacy, countsSql);
const nativeCounts = query(native, countsSql);
for (const table of tables) {
    if (legacyCounts[table] !== nativeCounts[table]) {
        console.error(`${table}: legacy=${legacyCounts[table]} rust=${nativeCounts[table]}`);
        mismatches++;
    }
}
for (const [name, sql] of Object.entries(relations)) {
    const before = query(legacy, sql);
    const after = query(native, sql);
    if (JSON.stringify(before) !== JSON.stringify(after)) {
        console.error(`${name}: named relationships differ`);
        mismatches++;
    }
}
if (mismatches) {
    console.error(`Seed parity failed: ${mismatches} difference(s).`);
    process.exit(1);
}
console.log(`Seed parity verified: ${tables.length} table counts and ${Object.keys(relations).length} named relationship sets.`);
