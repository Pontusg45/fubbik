CREATE TABLE projection_outbox (
    id text PRIMARY KEY,
    aggregate_type text NOT NULL,
    aggregate_id text NOT NULL,
    event_type text NOT NULL,
    payload jsonb NOT NULL,
    status text NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'completed', 'dead')),
    attempts integer NOT NULL DEFAULT 0,
    available_at timestamp without time zone NOT NULL DEFAULT now(),
    locked_at timestamp without time zone,
    last_error text,
    created_at timestamp without time zone NOT NULL DEFAULT now(),
    completed_at timestamp without time zone
);

CREATE INDEX projection_outbox_pending_idx
    ON projection_outbox (available_at, created_at)
    WHERE status = 'pending';

CREATE INDEX projection_outbox_aggregate_idx
    ON projection_outbox (aggregate_type, aggregate_id);
