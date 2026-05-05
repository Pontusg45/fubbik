-- Builtin vocabularies required for FK constraints (chunk.type → chunk_type, chunk_connection.relation → connection_relation).
-- Idempotent: safe if full seed ran earlier; aligns with packages/db/src/seed/modules/core.ts

INSERT INTO "chunk_type" ("id", "label", "description", "icon", "color", "examples", "display_order", "built_in")
VALUES
	('note', 'Note', 'A free-form note or observation', 'StickyNote', '#94a3b8',
	 '["Quick thought", "TODO", "Question"]'::jsonb, 10, true),
	('document', 'Document', 'Longer-form written content', 'FileText', '#3b82f6',
	 '["Spec", "RFC", "Meeting notes"]'::jsonb, 20, true),
	('guide', 'Guide', 'Step-by-step instructions or tutorial', 'BookOpen', '#6366f1',
	 '["Onboarding", "How-to"]'::jsonb, 30, true),
	('reference', 'Reference', 'Lookup material — APIs, glossary, canonical links', 'Compass', '#14b8a6',
	 '["API shape", "Glossary entry"]'::jsonb, 40, true),
	('schema', 'Schema', 'Data model or structural definition', 'Database', '#f59e0b',
	 '["Table schema", "Event payload"]'::jsonb, 50, true),
	('checklist', 'Checklist', 'Ordered items to verify or complete', 'CheckSquare', '#84cc16',
	 '["Launch checklist", "Review items"]'::jsonb, 60, true),
	('convention', 'Convention', 'A rule the team agrees to follow', 'Scale', '#ec4899',
	 '["Naming pattern", "Code style"]'::jsonb, 70, true)
ON CONFLICT ("id") DO UPDATE SET
	"label" = EXCLUDED."label",
	"description" = EXCLUDED."description",
	"icon" = EXCLUDED."icon",
	"color" = EXCLUDED."color",
	"examples" = EXCLUDED."examples",
	"display_order" = EXCLUDED."display_order",
	"built_in" = EXCLUDED."built_in";

INSERT INTO "connection_relation" ("id", "label", "description", "arrow_style", "direction", "color", "inverse_of_id", "display_order", "built_in")
VALUES
	('related_to', 'Related to', 'General relationship — the weakest link', 'dashed', 'bidirectional', '#94a3b8', NULL, 10, true),
	('part_of', 'Part of', 'Source is a component of target', 'solid', 'forward', '#3b82f6', NULL, 20, true),
	('contains', 'Contains', 'Source is a container holding target (inverse of part_of)', 'solid', 'forward', '#3b82f6', NULL, 21, true),
	('depends_on', 'Depends on', 'Source requires target to work', 'solid', 'forward', '#f59e0b', NULL, 30, true),
	('required_by', 'Required by', 'Target depends on source (inverse of depends_on)', 'solid', 'forward', '#f59e0b', NULL, 31, true),
	('extends', 'Extends', 'Source specializes or builds upon target', 'solid', 'forward', '#6366f1', NULL, 40, true),
	('extended_by', 'Extended by', 'Target extends source (inverse of extends)', 'solid', 'forward', '#6366f1', NULL, 41, true),
	('references', 'References', 'Source mentions or cites target', 'dotted', 'forward', '#14b8a6', NULL, 50, true),
	('referenced_by', 'Referenced by', 'Target references source (inverse of references)', 'dotted', 'forward', '#14b8a6', NULL, 51, true),
	('supports', 'Supports', 'Source provides evidence for target', 'solid', 'forward', '#22c55e', NULL, 60, true),
	('supported_by', 'Supported by', 'Target supports source (inverse of supports)', 'solid', 'forward', '#22c55e', NULL, 61, true),
	('contradicts', 'Contradicts', 'Source disagrees with target', 'solid', 'bidirectional', '#ef4444', NULL, 70, true),
	('alternative_to', 'Alternative to', 'Source and target are competing approaches', 'dashed', 'bidirectional', '#a855f7', NULL, 80, true)
ON CONFLICT ("id") DO UPDATE SET
	"label" = EXCLUDED."label",
	"description" = EXCLUDED."description",
	"arrow_style" = EXCLUDED."arrow_style",
	"direction" = EXCLUDED."direction",
	"color" = EXCLUDED."color",
	"display_order" = EXCLUDED."display_order",
	"built_in" = EXCLUDED."built_in";

UPDATE "connection_relation" SET "inverse_of_id" = 'required_by' WHERE "id" = 'depends_on';
UPDATE "connection_relation" SET "inverse_of_id" = 'depends_on' WHERE "id" = 'required_by';
UPDATE "connection_relation" SET "inverse_of_id" = 'contains' WHERE "id" = 'part_of';
UPDATE "connection_relation" SET "inverse_of_id" = 'part_of' WHERE "id" = 'contains';
UPDATE "connection_relation" SET "inverse_of_id" = 'extended_by' WHERE "id" = 'extends';
UPDATE "connection_relation" SET "inverse_of_id" = 'extends' WHERE "id" = 'extended_by';
UPDATE "connection_relation" SET "inverse_of_id" = 'referenced_by' WHERE "id" = 'references';
UPDATE "connection_relation" SET "inverse_of_id" = 'references' WHERE "id" = 'referenced_by';
UPDATE "connection_relation" SET "inverse_of_id" = 'supported_by' WHERE "id" = 'supports';
UPDATE "connection_relation" SET "inverse_of_id" = 'supports' WHERE "id" = 'supported_by';
