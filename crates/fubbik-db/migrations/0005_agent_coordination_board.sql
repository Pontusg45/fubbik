ALTER TABLE plan_task
    ADD CONSTRAINT plan_task_id_plan_id_unique UNIQUE (id, plan_id);

CREATE TABLE agent_run (
    id text PRIMARY KEY,
    plan_id text NOT NULL REFERENCES plan(id) ON DELETE CASCADE,
    parent_run_id text,
    handle text NOT NULL CHECK (btrim(handle) <> ''),
    external_key text,
    status text NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'finished', 'abandoned')),
    capabilities jsonb NOT NULL DEFAULT '[]'::jsonb,
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    last_ack_sequence bigint NOT NULL DEFAULT 0 CHECK (last_ack_sequence >= 0),
    last_heartbeat_at timestamp without time zone NOT NULL DEFAULT now(),
    created_at timestamp without time zone NOT NULL DEFAULT now(),
    updated_at timestamp without time zone NOT NULL DEFAULT now(),
    CONSTRAINT agent_run_id_plan_id_unique UNIQUE (id, plan_id),
    CONSTRAINT agent_run_parent_same_plan_fk
        FOREIGN KEY (parent_run_id, plan_id)
        REFERENCES agent_run(id, plan_id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX agent_run_plan_external_key_unique_idx
    ON agent_run(plan_id, external_key)
    WHERE external_key IS NOT NULL;
CREATE INDEX agent_run_plan_status_idx ON agent_run(plan_id, status);
CREATE INDEX agent_run_parent_idx ON agent_run(parent_run_id);

CREATE TABLE plan_task_claim (
    task_id text PRIMARY KEY,
    plan_id text NOT NULL,
    agent_run_id text NOT NULL,
    claimed_at timestamp without time zone NOT NULL DEFAULT now(),
    lease_expires_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone NOT NULL DEFAULT now(),
    CONSTRAINT plan_task_claim_task_same_plan_fk
        FOREIGN KEY (task_id, plan_id)
        REFERENCES plan_task(id, plan_id) ON DELETE CASCADE,
    CONSTRAINT plan_task_claim_run_same_plan_fk
        FOREIGN KEY (agent_run_id, plan_id)
        REFERENCES agent_run(id, plan_id) ON DELETE CASCADE
);

CREATE INDEX plan_task_claim_plan_lease_idx
    ON plan_task_claim(plan_id, lease_expires_at);

CREATE TABLE coordination_entry (
    id text PRIMARY KEY,
    sequence bigint GENERATED ALWAYS AS IDENTITY UNIQUE,
    plan_id text NOT NULL REFERENCES plan(id) ON DELETE CASCADE,
    task_id text,
    author_run_id text NOT NULL,
    recipient_run_id text,
    reply_to_id text,
    kind text NOT NULL
        CHECK (kind IN ('note', 'question', 'answer', 'progress', 'decision', 'handoff', 'artifact', 'system')),
    body text NOT NULL CHECK (btrim(body) <> ''),
    metadata jsonb NOT NULL DEFAULT '{}'::jsonb,
    client_mutation_id text NOT NULL CHECK (btrim(client_mutation_id) <> ''),
    created_at timestamp without time zone NOT NULL DEFAULT now(),
    CONSTRAINT coordination_entry_id_plan_id_unique UNIQUE (id, plan_id),
    CONSTRAINT coordination_entry_author_mutation_unique UNIQUE (author_run_id, client_mutation_id),
    CONSTRAINT coordination_entry_task_same_plan_fk
        FOREIGN KEY (task_id, plan_id)
        REFERENCES plan_task(id, plan_id) ON DELETE CASCADE,
    CONSTRAINT coordination_entry_author_same_plan_fk
        FOREIGN KEY (author_run_id, plan_id)
        REFERENCES agent_run(id, plan_id) ON DELETE CASCADE,
    CONSTRAINT coordination_entry_recipient_same_plan_fk
        FOREIGN KEY (recipient_run_id, plan_id)
        REFERENCES agent_run(id, plan_id) ON DELETE CASCADE,
    CONSTRAINT coordination_entry_reply_same_plan_fk
        FOREIGN KEY (reply_to_id, plan_id)
        REFERENCES coordination_entry(id, plan_id) ON DELETE CASCADE
);

CREATE INDEX coordination_entry_plan_sequence_idx
    ON coordination_entry(plan_id, sequence);
CREATE INDEX coordination_entry_recipient_sequence_idx
    ON coordination_entry(recipient_run_id, sequence);
CREATE INDEX coordination_entry_task_sequence_idx
    ON coordination_entry(task_id, sequence);
