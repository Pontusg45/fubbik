-- `chunk_version.snapshot()` derives the version number inside a single
-- INSERT (COALESCE(MAX(version), 0) + 1) to avoid a client-side
-- read-then-write gap, but under READ COMMITTED that is not atomic against
-- a second concurrent transaction: two overlapping snapshots for the same
-- chunk can each see the same prior max and insert the same version.
--
-- This constraint is what actually prevents duplicate version numbers per
-- chunk — a concurrent same-chunk snapshot now fails loudly with a unique
-- violation instead of silently corrupting history.
--
-- Deduplicate first so the constraint can be created even if a duplicate
-- already slipped in before this migration existed: keep the earliest row
-- (by created_at, tie-broken by id) for each (chunk_id, version) pair and
-- drop the rest.
DELETE FROM chunk_version cv
USING chunk_version keep
WHERE cv.chunk_id = keep.chunk_id
  AND cv.version = keep.version
  AND (cv.created_at, cv.id) > (keep.created_at, keep.id);

ALTER TABLE chunk_version
    ADD CONSTRAINT chunk_version_chunk_id_version_key UNIQUE (chunk_id, version);
