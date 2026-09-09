-- Migration 0001 historically set a session-scoped search_path while
-- initializing Apache AGE. On AGE-enabled clusters, the same migration
-- connection then created the unqualified relations from migrations 0005-0007
-- in ag_catalog. Restore the application schema and leave the connection safe
-- for every migration that follows this one.
SET search_path = "$user", public;

DO $$
DECLARE
    relation_name text;
BEGIN
    FOREACH relation_name IN ARRAY ARRAY[
        'agent_run',
        'plan_task_claim',
        'coordination_entry',
        'projection_outbox',
        'account'
    ]
    LOOP
        IF to_regclass(format('public.%I', relation_name)) IS NOT NULL
           AND to_regclass(format('ag_catalog.%I', relation_name)) IS NOT NULL THEN
            RAISE EXCEPTION
                'cannot repair relation %: it exists in both public and ag_catalog',
                relation_name;
        ELSIF to_regclass(format('public.%I', relation_name)) IS NULL
              AND to_regclass(format('ag_catalog.%I', relation_name)) IS NOT NULL THEN
            EXECUTE format(
                'ALTER TABLE ag_catalog.%I SET SCHEMA public',
                relation_name
            );
        ELSIF to_regclass(format('public.%I', relation_name)) IS NULL THEN
            RAISE EXCEPTION
                'cannot repair relation %: it is missing from both public and ag_catalog',
                relation_name;
        END IF;
    END LOOP;
END $$;

