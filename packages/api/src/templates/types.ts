// packages/api/src/templates/types.ts

// --- Match Rules ---

export interface HeadingRule {
  /** One or more heading text patterns (alternatives — any match counts) */
  patterns: string[];
  /** How to match. Default: "prefix" */
  match: "exact" | "prefix" | "contains";
  /** Heading level (2 = ##, 3 = ###). Omit to match any level. */
  level?: number;
  /** If true, doc must have this heading to match. Default: true */
  required: boolean;
}

export interface FrontmatterRule {
  /** Frontmatter key to check */
  key: string;
  /** Match mode. Default: "exact" */
  match: "exact" | "oneOf" | "exists";
  /** Expected value for "exact" mode */
  value?: string;
  /** Expected values for "oneOf" mode */
  values?: string[];
}

export interface MatchRules {
  /** Minimum total score to qualify as a match */
  minScore: number;
  /** Heading patterns to look for */
  headings: HeadingRule[];
  /** Frontmatter field expectations */
  frontmatter: FrontmatterRule[];
}

// --- Field Extraction ---

export type ExtractionTarget =
  | "rationale"
  | "alternatives"
  | "consequences"
  | "summary"
  | "scope"
  | "content";

export interface FieldMapping {
  /** Heading patterns to match (same alias approach as matchRules) */
  headings: string[];
  /** How to match. Default: "prefix" */
  match: "exact" | "prefix" | "contains";
  /** Chunk field to populate */
  target: ExtractionTarget;
}

// --- Matching result ---

export interface ParsedHeading {
  text: string;
  level: number;
}

export interface TemplateMatch {
  templateId: string;
  templateName: string;
  score: number;
  type: string;
  tags: string[];
  extractedFields: ExtractedFields;
}

export interface ExtractedFields {
  rationale?: string;
  alternatives?: string[];
  consequences?: string;
  summary?: string;
  scope?: Record<string, string>;
  content?: string;
}

export interface TemplateWithRules {
  id: string;
  name: string;
  type: string;
  matchRules: MatchRules;
  fieldMappings: FieldMapping[] | null;
  priority: number;
  tags: string[] | null;
}
