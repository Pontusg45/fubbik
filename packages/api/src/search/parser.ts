import type { QueryClause } from "./types.js";

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

/**
 * Splits a query string into tokens, respecting double-quoted strings.
 * Quoted strings are returned with their quotes stripped.
 * e.g. `type:reference "bare text" NOT tag:api` → ['type:reference', 'bare text', 'NOT', 'tag:api']
 */
function tokenize(input: string): string[] {
  const tokens: string[] = [];
  let i = 0;
  const len = input.length;

  while (i < len) {
    // Skip whitespace
    while (i < len && /\s/.test(input[i]!)) i++;
    if (i >= len) break;

    if (input[i] === '"') {
      // Quoted token — consume until closing quote
      i++; // skip opening quote
      let token = "";
      while (i < len && input[i] !== '"') {
        token += input[i++];
      }
      if (i < len) i++; // skip closing quote
      tokens.push(token);
    } else {
      // Unquoted token — consume until whitespace
      let token = "";
      while (i < len && !/\s/.test(input[i]!)) {
        token += input[i++];
      }
      tokens.push(token);
    }
  }

  return tokens;
}

// ---------------------------------------------------------------------------
// Field-specific operator detection
// ---------------------------------------------------------------------------

/**
 * Given a raw field:rawValue token (after splitting on first ':'), determine
 * the appropriate operator and normalise the value.
 */
function resolveOperatorAndValue(
  field: string,
  rawValue: string,
): { operator: string; value: string; params?: Record<string, string> } {
  // connections:3+  → gte
  if (field === "connections" && rawValue.endsWith("+")) {
    return { operator: "gte", value: rawValue.slice(0, -1) };
  }

  // updated:30d  → within (strip trailing 'd')
  if (field === "updated" && /^\d+d$/.test(rawValue)) {
    return { operator: "within", value: rawValue.slice(0, -1) };
  }

  // tag:api,auth  → any_of
  if (field === "tag" && rawValue.includes(",")) {
    return { operator: "any_of", value: rawValue };
  }

  // path:"A"->"B"  — e.g. rawValue might be `"A"->"B"` (already unquoted first segment)
  // After tokenization the token looks like: path:A->"B" or the arrow+dest is separate.
  // We handle the path:"A"->"B" form where the arrow is embedded within the value.
  if (field === "path" && rawValue.includes("->")) {
    const arrowIdx = rawValue.indexOf("->");
    const from = rawValue.slice(0, arrowIdx).replace(/^"|"$/g, "");
    const to = rawValue.slice(arrowIdx + 2).replace(/^"|"$/g, "");
    return { operator: "is", value: from, params: { from, to } };
  }

  return { operator: "is", value: rawValue };
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/**
 * Parses a structured query string into an array of QueryClause objects.
 *
 * Supported syntax:
 *   type:reference
 *   tag:api
 *   tag:api,auth            → any_of
 *   connections:3+          → gte
 *   updated:30d             → within
 *   near:"Auth Flow" hops:2 → near clause with params.hops
 *   path:"A"->"B"           → path clause with params.from/to
 *   NOT tag:deprecated      → negate: true
 *   bare words / "quoted"   → { field: "text", operator: "contains" }
 */
export function parseQueryString(input: string): QueryClause[] {
  const tokens = tokenize(input);
  const clauses: QueryClause[] = [];
  let negate = false;

  for (let i = 0; i < tokens.length; i++) {
    const token = tokens[i]!;

    // NOT prefix
    if (token === "NOT") {
      negate = true;
      continue;
    }

    // hops:N — attach as param to the last "near" clause
    if (/^hops:\d+$/.test(token)) {
      const hopsValue = token.slice(5); // strip "hops:"
      const lastNear = [...clauses].reverse().find((c) => c.field === "near");
      if (lastNear) {
        lastNear.params = { ...lastNear.params, hops: hopsValue };
      }
      negate = false;
      continue;
    }

    // field:value token
    const colonIdx = token.indexOf(":");
    if (colonIdx > 0) {
      const field = token.slice(0, colonIdx);
      // The raw value may itself contain a quoted segment that was collapsed by
      // the tokenizer (e.g. `path:"A"->"B"` arrives as path:A->"B" because
      // only leading/trailing quotes are stripped by tokenize).  Re-strip here.
      const rawValue = token.slice(colonIdx + 1).replace(/^"|"$/g, "");

      const { operator, value, params } = resolveOperatorAndValue(
        field,
        rawValue,
      );

      const clause: QueryClause = { field, operator, value, negate };
      if (params) clause.params = params;
      if (!negate) delete clause.negate;

      clauses.push(clause);
      negate = false;
      continue;
    }

    // Bare text (already unquoted by tokenizer if it was quoted in input)
    const clause: QueryClause = {
      field: "text",
      operator: "contains",
      value: token,
      ...(negate ? { negate: true } : {}),
    };
    clauses.push(clause);
    negate = false;
  }

  return clauses;
}

// ---------------------------------------------------------------------------
// Serializer — inverse of parseQueryString
// ---------------------------------------------------------------------------

/**
 * Converts an array of QueryClause objects back to a query string.
 * Used to sync pill state → text input.
 */
export function clausesToQueryString(clauses: QueryClause[]): string {
  return clauses
    .map((clause) => {
      const prefix = clause.negate ? "NOT " : "";

      if (clause.field === "text") {
        const v = clause.value.includes(" ")
          ? `"${clause.value}"`
          : clause.value;
        return `${prefix}${v}`;
      }

      if (clause.field === "path" && clause.params?.from && clause.params?.to) {
        const from = clause.params.from.includes(" ")
          ? `"${clause.params.from}"`
          : clause.params.from;
        const to = clause.params.to.includes(" ")
          ? `"${clause.params.to}"`
          : clause.params.to;
        return `${prefix}path:${from}->${to}`;
      }

      if (clause.field === "near") {
        const v = clause.value.includes(" ")
          ? `"${clause.value}"`
          : clause.value;
        let result = `${prefix}near:${v}`;
        if (clause.params?.hops) {
          result += ` hops:${clause.params.hops}`;
        }
        return result;
      }

      if (clause.operator === "gte" && clause.field === "connections") {
        return `${prefix}${clause.field}:${clause.value}+`;
      }

      if (clause.operator === "within" && clause.field === "updated") {
        return `${prefix}${clause.field}:${clause.value}d`;
      }

      const v = clause.value.includes(" ")
        ? `"${clause.value}"`
        : clause.value;
      return `${prefix}${clause.field}:${v}`;
    })
    .join(" ");
}
