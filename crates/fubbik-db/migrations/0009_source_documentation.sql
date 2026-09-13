-- Import ownership and the last generated text are separate from editable chunks.
CREATE TABLE source_documentation (
    document_id text PRIMARY KEY REFERENCES document(id) ON DELETE CASCADE,
    user_id text NOT NULL REFERENCES "user"(id) ON DELETE CASCADE,
    space_id text NOT NULL REFERENCES space(id) ON DELETE CASCADE,
    project text NOT NULL,
    language text NOT NULL,
    extractor text NOT NULL,
    UNIQUE (user_id, space_id, project, language)
);

CREATE TABLE source_documentation_symbol (
    document_id text NOT NULL REFERENCES source_documentation(document_id) ON DELETE CASCADE,
    symbol_key text NOT NULL,
    chunk_id text NOT NULL UNIQUE REFERENCES chunk(id) ON DELETE CASCADE,
    generated_title text NOT NULL,
    generated_content text NOT NULL,
    missing boolean NOT NULL DEFAULT false,
    PRIMARY KEY (document_id, symbol_key)
);
