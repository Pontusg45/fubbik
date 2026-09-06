import { getTableConfig, type PgTable } from "drizzle-orm/pg-core";
import { Pool } from "pg";

import * as schema from "./schema/index";

export interface SchemaColumn {
    name: string;
    sqlType: string;
    notNull: boolean;
}

export interface SchemaTable {
    name: string;
    columns: SchemaColumn[];
}

export interface SchemaDriftAllowance {
    issue: string;
    reason: string;
}

export const SCHEMA_DRIFT_ALLOWLIST: readonly SchemaDriftAllowance[] = [
    { issue: "account exists only in Drizzle", reason: "Better Auth legacy table; Rust auth stores password_hash on user" },
    { issue: "verification exists only in Drizzle", reason: "Better Auth legacy table; Rust auth does not use verification tokens" },
    { issue: "graph_event exists only in Drizzle", reason: "legacy Node graph-event repository pending backend deletion" },
    { issue: "usage_event exists only in Drizzle", reason: "legacy Node usage repository pending backend deletion" },
    { issue: "user.password_hash exists only in SQL", reason: "Rust-owned password authentication column" },
    { issue: "projection_outbox exists only in SQL", reason: "Rust-owned durable AGE projection queue" }
];

function normalizeType(sqlType: string): string {
    const normalized = sqlType.toLowerCase().replaceAll(/\s+/g, " ").trim();
    if (normalized === "timestamp" || normalized.startsWith("timestamp(")) return "timestamp without time zone";
    if (normalized === "serial") return "integer";
    if (normalized === "bigserial") return "bigint";
    return normalized;
}

export function compareSchemas(drizzleTables: readonly SchemaTable[], sqlTables: readonly SchemaTable[]): string[] {
    const issues: string[] = [];
    const drizzleByName = new Map(drizzleTables.map(table => [table.name, table]));
    const sqlByName = new Map(sqlTables.map(table => [table.name, table]));

    for (const [tableName, drizzleTable] of drizzleByName) {
        const sqlTable = sqlByName.get(tableName);
        if (!sqlTable) {
            issues.push(`${tableName} exists only in Drizzle`);
            continue;
        }
        const drizzleColumns = new Map(drizzleTable.columns.map(column => [column.name, column]));
        const sqlColumns = new Map(sqlTable.columns.map(column => [column.name, column]));
        for (const [columnName, drizzleColumn] of drizzleColumns) {
            const sqlColumn = sqlColumns.get(columnName);
            if (!sqlColumn) {
                issues.push(`${tableName}.${columnName} exists only in Drizzle`);
                continue;
            }
            if (normalizeType(drizzleColumn.sqlType) !== normalizeType(sqlColumn.sqlType)) {
                issues.push(`${tableName}.${columnName} type: Drizzle=${drizzleColumn.sqlType} SQL=${sqlColumn.sqlType}`);
            }
            if (drizzleColumn.notNull !== sqlColumn.notNull) {
                issues.push(
                    `${tableName}.${columnName} nullability: Drizzle=${drizzleColumn.notNull ? "not null" : "nullable"} SQL=${sqlColumn.notNull ? "not null" : "nullable"}`
                );
            }
        }
        for (const columnName of sqlColumns.keys()) {
            if (!drizzleColumns.has(columnName)) issues.push(`${tableName}.${columnName} exists only in SQL`);
        }
    }
    for (const tableName of sqlByName.keys()) {
        if (!drizzleByName.has(tableName) && tableName !== "_sqlx_migrations") {
            issues.push(`${tableName} exists only in SQL`);
        }
    }
    return issues.sort();
}

export function assertSchemaParity(issues: readonly string[], allowances: readonly SchemaDriftAllowance[]): void {
    const allowed = new Set(allowances.map(allowance => allowance.issue));
    const unexpected = issues.filter(issue => !allowed.has(issue));
    if (unexpected.length > 0) {
        throw new Error(`SQL/Drizzle schema drift:\n${unexpected.map(issue => `- ${issue}`).join("\n")}`);
    }
}

export function readDrizzleSchema(): SchemaTable[] {
    const tables = new Map<string, SchemaTable>();
    for (const value of Object.values(schema)) {
        try {
            const config = getTableConfig(value as PgTable);
            if (!config.name || tables.has(config.name)) continue;
            tables.set(config.name, {
                name: config.name,
                columns: config.columns.map(column => ({
                    name: column.name,
                    sqlType: normalizeType(column.getSQLType()),
                    notNull: column.notNull
                }))
            });
        } catch {
            // Relations and helper exports are intentionally not tables.
        }
    }
    return [...tables.values()].sort((left, right) => left.name.localeCompare(right.name));
}

export async function readSqlSchema(databaseUrl: string): Promise<SchemaTable[]> {
    const pool = new Pool({ connectionString: databaseUrl });
    try {
        const result = await pool.query<{
            table_name: string;
            column_name: string;
            sql_type: string;
            not_null: boolean;
        }>(`
            SELECT c.relname AS table_name,
                   a.attname AS column_name,
                   pg_catalog.format_type(a.atttypid, a.atttypmod) AS sql_type,
                   a.attnotnull AS not_null
            FROM pg_catalog.pg_attribute a
            JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
            JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname = 'public'
              AND c.relkind IN ('r', 'p')
              AND a.attnum > 0
              AND NOT a.attisdropped
            ORDER BY c.relname, a.attnum
        `);
        const tables = new Map<string, SchemaTable>();
        for (const row of result.rows) {
            const table = tables.get(row.table_name) ?? { name: row.table_name, columns: [] };
            table.columns.push({
                name: row.column_name,
                sqlType: normalizeType(row.sql_type),
                notNull: row.not_null
            });
            tables.set(row.table_name, table);
        }
        return [...tables.values()];
    } finally {
        await pool.end();
    }
}

export async function verifySchema(databaseUrl: string): Promise<void> {
    const issues = compareSchemas(readDrizzleSchema(), await readSqlSchema(databaseUrl));
    assertSchemaParity(issues, SCHEMA_DRIFT_ALLOWLIST);
}
