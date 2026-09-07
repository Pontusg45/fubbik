-- Preserve credentials created by the former Better Auth / Drizzle server
-- long enough for the Rust sign-in route to verify and transparently
-- upgrade them into user.password_hash. IF NOT EXISTS keeps this safe on
-- databases where Better Auth already created the table.
CREATE TABLE IF NOT EXISTS account (
    id text PRIMARY KEY,
    account_id text NOT NULL,
    provider_id text NOT NULL,
    user_id text NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    access_token text,
    refresh_token text,
    id_token text,
    access_token_expires_at timestamp without time zone,
    refresh_token_expires_at timestamp without time zone,
    scope text,
    password text,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);

CREATE INDEX IF NOT EXISTS "account_userId_idx" ON account(user_id);
