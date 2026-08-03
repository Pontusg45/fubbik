CREATE EXTENSION IF NOT EXISTS vector;
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- AGE is optional. Swallow the failure so a Postgres without the extension
-- still migrates cleanly; fubbik_db::age degrades to empty results.
DO $$
BEGIN
    CREATE EXTENSION IF NOT EXISTS age;
    LOAD 'age';
    SET search_path = ag_catalog, "$user", public;
    PERFORM ag_catalog.create_graph('knowledge');
EXCEPTION
    WHEN duplicate_schema THEN NULL;  -- graph already exists
    WHEN OTHERS THEN
        RAISE NOTICE 'AGE unavailable, graph features disabled: %', SQLERRM;
END $$;

--
-- Name: activity_log; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.activity_log (
    id text NOT NULL,
    user_id text NOT NULL,
    entity_type text NOT NULL,
    entity_id text NOT NULL,
    entity_title text,
    action text NOT NULL,
    space_id text,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: behavior_cell; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.behavior_cell (
    id text NOT NULL,
    rule_id text NOT NULL,
    dimension_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: behavior_cell_requirement; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.behavior_cell_requirement (
    cell_id text NOT NULL,
    requirement_id text NOT NULL
);


--
-- Name: behavior_dimension; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.behavior_dimension (
    id text NOT NULL,
    matrix_id text NOT NULL,
    name text NOT NULL,
    "order" integer DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: behavior_matrix; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.behavior_matrix (
    id text NOT NULL,
    name text NOT NULL,
    layer text NOT NULL,
    description text,
    space_id text,
    user_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: behavior_rule; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.behavior_rule (
    id text NOT NULL,
    matrix_id text NOT NULL,
    title text NOT NULL,
    description text,
    category text,
    "order" integer DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: chunk; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk (
    id text NOT NULL,
    title text NOT NULL,
    content text DEFAULT ''::text NOT NULL,
    type text DEFAULT 'note'::text NOT NULL,
    user_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL,
    summary text,
    aliases jsonb DEFAULT '[]'::jsonb NOT NULL,
    not_about jsonb DEFAULT '[]'::jsonb NOT NULL,
    scope jsonb DEFAULT '{}'::jsonb NOT NULL,
    rationale text,
    alternatives jsonb,
    consequences text,
    embedding public.vector(768),
    embedding_updated_at timestamp without time zone,
    origin text DEFAULT 'human'::text NOT NULL,
    review_status text DEFAULT 'approved'::text NOT NULL,
    reviewed_by text,
    reviewed_at timestamp without time zone,
    archived_at timestamp without time zone,
    document_id text,
    document_order integer,
    is_entry_point boolean DEFAULT false NOT NULL
);


--
-- Name: chunk_applies_to; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk_applies_to (
    id text NOT NULL,
    chunk_id text NOT NULL,
    pattern text NOT NULL,
    note text
);


--
-- Name: chunk_comment; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk_comment (
    id text NOT NULL,
    chunk_id text NOT NULL,
    user_id text NOT NULL,
    content text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: chunk_connection; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk_connection (
    id text NOT NULL,
    source_id text NOT NULL,
    target_id text NOT NULL,
    relation text DEFAULT 'related_to'::text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    origin text DEFAULT 'human'::text NOT NULL,
    review_status text DEFAULT 'approved'::text NOT NULL,
    reviewed_by text,
    reviewed_at timestamp without time zone,
    weight integer DEFAULT 1 NOT NULL
);


--
-- Name: chunk_feature_delta; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk_feature_delta (
    id text NOT NULL,
    chunk_id text NOT NULL,
    feature_id text NOT NULL,
    delta jsonb NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: chunk_file_ref; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk_file_ref (
    id text NOT NULL,
    chunk_id text NOT NULL,
    path text NOT NULL,
    anchor text,
    relation text DEFAULT 'documents'::text NOT NULL
);


--
-- Name: chunk_proposal; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk_proposal (
    id text NOT NULL,
    chunk_id text NOT NULL,
    changes jsonb NOT NULL,
    reason text,
    status text DEFAULT 'pending'::text NOT NULL,
    proposed_by text NOT NULL,
    reviewed_by text,
    reviewed_at timestamp without time zone,
    review_note text,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: chunk_space; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk_space (
    chunk_id text NOT NULL,
    space_id text NOT NULL
);


--
-- Name: chunk_staleness; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk_staleness (
    id text NOT NULL,
    chunk_id text NOT NULL,
    reason text NOT NULL,
    detail text,
    related_chunk_id text,
    detected_at timestamp without time zone DEFAULT now() NOT NULL,
    dismissed_at timestamp without time zone,
    dismissed_by text,
    suppress_pair text
);


--
-- Name: chunk_tag; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk_tag (
    chunk_id text NOT NULL,
    tag_id text NOT NULL
);


--
-- Name: chunk_template; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk_template (
    id text NOT NULL,
    name text NOT NULL,
    description text,
    type text DEFAULT 'note'::text NOT NULL,
    content text DEFAULT ''::text NOT NULL,
    is_built_in boolean DEFAULT false NOT NULL,
    match_rules jsonb,
    field_mappings jsonb,
    priority integer DEFAULT 0 NOT NULL,
    tags text[],
    user_id text,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: chunk_type; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk_type (
    id text NOT NULL,
    label text NOT NULL,
    description text,
    icon text,
    color text DEFAULT '#8b5cf6'::text NOT NULL,
    examples jsonb DEFAULT '[]'::jsonb NOT NULL,
    display_order integer DEFAULT 100 NOT NULL,
    built_in boolean DEFAULT false NOT NULL,
    user_id text,
    space_id text,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: chunk_version; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.chunk_version (
    id text NOT NULL,
    chunk_id text NOT NULL,
    version integer NOT NULL,
    title text NOT NULL,
    content text NOT NULL,
    type text NOT NULL,
    tags jsonb NOT NULL,
    rationale text,
    alternatives jsonb,
    consequences text,
    scope jsonb,
    update_tag text,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: codebase_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.codebase_settings (
    id text NOT NULL,
    space_id text NOT NULL,
    key text NOT NULL,
    value jsonb NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: collection; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.collection (
    id text NOT NULL,
    name text NOT NULL,
    description text,
    filter jsonb NOT NULL,
    user_id text NOT NULL,
    space_id text,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: connection_relation; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.connection_relation (
    id text NOT NULL,
    label text NOT NULL,
    description text,
    arrow_style text DEFAULT 'solid'::text NOT NULL,
    direction text DEFAULT 'forward'::text NOT NULL,
    color text DEFAULT '#64748b'::text NOT NULL,
    inverse_of_id text,
    display_order integer DEFAULT 100 NOT NULL,
    built_in boolean DEFAULT false NOT NULL,
    user_id text,
    space_id text,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: context_snapshot; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.context_snapshot (
    id text NOT NULL,
    user_id text NOT NULL,
    query jsonb NOT NULL,
    chunks jsonb NOT NULL,
    token_count integer NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: document; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.document (
    id text NOT NULL,
    title text NOT NULL,
    source_path text NOT NULL,
    content_hash text NOT NULL,
    description text,
    split_level integer,
    space_id text,
    user_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: feature; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.feature (
    id text NOT NULL,
    name text NOT NULL,
    description text,
    priority integer NOT NULL,
    status text DEFAULT 'inactive'::text NOT NULL,
    color text,
    user_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: feature_space; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.feature_space (
    feature_id text NOT NULL,
    space_id text NOT NULL
);


--
-- Name: instance_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.instance_settings (
    key text NOT NULL,
    value jsonb NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: learning_path; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.learning_path (
    id text NOT NULL,
    title text NOT NULL,
    description text,
    chunk_ids jsonb DEFAULT '[]'::jsonb NOT NULL,
    user_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: notification; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.notification (
    id text NOT NULL,
    user_id text NOT NULL,
    type text NOT NULL,
    title text NOT NULL,
    message text NOT NULL,
    link_to text,
    read boolean DEFAULT false NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: plan; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.plan (
    id text NOT NULL,
    title text NOT NULL,
    description text,
    status text DEFAULT 'draft'::text NOT NULL,
    user_id text NOT NULL,
    space_id text,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL,
    completed_at timestamp without time zone,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL
);


--
-- Name: plan_analyze_item; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.plan_analyze_item (
    id text NOT NULL,
    plan_id text NOT NULL,
    kind text NOT NULL,
    "order" integer DEFAULT 0 NOT NULL,
    chunk_id text,
    file_path text,
    text text,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: plan_external_link; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.plan_external_link (
    id text NOT NULL,
    plan_id text NOT NULL,
    system text DEFAULT 'url'::text NOT NULL,
    url text NOT NULL,
    label text,
    "order" integer DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: plan_requirement; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.plan_requirement (
    id text NOT NULL,
    plan_id text NOT NULL,
    requirement_id text NOT NULL,
    "order" integer DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: plan_task; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.plan_task (
    id text NOT NULL,
    plan_id text NOT NULL,
    title text NOT NULL,
    description text,
    acceptance_criteria jsonb DEFAULT '[]'::jsonb NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    "order" integer DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL
);


--
-- Name: plan_task_chunk; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.plan_task_chunk (
    id text NOT NULL,
    task_id text NOT NULL,
    chunk_id text NOT NULL,
    relation text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: plan_task_dependency; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.plan_task_dependency (
    id text NOT NULL,
    task_id text NOT NULL,
    depends_on_task_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: plan_task_external_link; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.plan_task_external_link (
    id text NOT NULL,
    task_id text NOT NULL,
    system text DEFAULT 'url'::text NOT NULL,
    url text NOT NULL,
    label text,
    "order" integer DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: requirement; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.requirement (
    id text NOT NULL,
    title text NOT NULL,
    description text,
    steps jsonb NOT NULL,
    "order" integer DEFAULT 0 NOT NULL,
    status text DEFAULT 'untested'::text NOT NULL,
    priority text,
    space_id text,
    user_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL,
    origin text DEFAULT 'human'::text NOT NULL,
    review_status text DEFAULT 'approved'::text NOT NULL,
    use_case_id text,
    reviewed_by text,
    reviewed_at timestamp without time zone
);


--
-- Name: requirement_chunk; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.requirement_chunk (
    requirement_id text NOT NULL,
    chunk_id text NOT NULL
);


--
-- Name: requirement_dependency; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.requirement_dependency (
    requirement_id text NOT NULL,
    depends_on_id text NOT NULL,
    CONSTRAINT no_self_dependency CHECK ((requirement_id <> depends_on_id))
);


--
-- Name: saved_graph; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.saved_graph (
    id text NOT NULL,
    name text NOT NULL,
    description text,
    chunk_ids jsonb DEFAULT '[]'::jsonb NOT NULL,
    positions jsonb DEFAULT '{}'::jsonb NOT NULL,
    layout_algorithm text DEFAULT 'force'::text NOT NULL,
    user_id text NOT NULL,
    space_id text,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: saved_query; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.saved_query (
    id text NOT NULL,
    name text NOT NULL,
    query jsonb NOT NULL,
    user_id text NOT NULL,
    space_id text,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: scope_key; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.scope_key (
    id text NOT NULL,
    user_id text NOT NULL,
    key text NOT NULL,
    description text,
    value_type text DEFAULT 'string'::text NOT NULL,
    allowed_values jsonb,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: session; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.session (
    id text NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    token text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone NOT NULL,
    ip_address text,
    user_agent text,
    user_id text NOT NULL
);


--
-- Name: space; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.space (
    id text NOT NULL,
    name text NOT NULL,
    kind text NOT NULL,
    description text,
    user_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: space_code_metadata; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.space_code_metadata (
    space_id text NOT NULL,
    user_id text NOT NULL,
    remote_url text,
    local_paths jsonb DEFAULT '[]'::jsonb NOT NULL
);


--
-- Name: space_kind; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.space_kind (
    id text NOT NULL,
    label text NOT NULL,
    description text,
    icon text,
    display_order integer DEFAULT 100 NOT NULL,
    built_in boolean DEFAULT false NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: staleness_scan; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.staleness_scan (
    id text NOT NULL,
    space_id text NOT NULL,
    last_commit_sha text NOT NULL,
    scanned_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: tag; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.tag (
    id text NOT NULL,
    name text NOT NULL,
    tag_type_id text,
    user_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    origin text DEFAULT 'human'::text NOT NULL,
    review_status text DEFAULT 'approved'::text NOT NULL,
    reviewed_by text,
    reviewed_at timestamp without time zone
);


--
-- Name: tag_type; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.tag_type (
    id text NOT NULL,
    name text NOT NULL,
    color text DEFAULT '#8b5cf6'::text NOT NULL,
    icon text,
    user_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: use_case; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.use_case (
    id text NOT NULL,
    name text NOT NULL,
    description text,
    space_id text,
    user_id text NOT NULL,
    "order" integer DEFAULT 0 NOT NULL,
    parent_id text,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: user; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public."user" (
    id text NOT NULL,
    name text NOT NULL,
    email text NOT NULL,
    email_verified boolean DEFAULT false NOT NULL,
    image text,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL,
    password_hash text
);


--
-- Name: user_active_feature; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_active_feature (
    user_id text NOT NULL,
    feature_id text NOT NULL
);


--
-- Name: user_favorite; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_favorite (
    id text NOT NULL,
    user_id text NOT NULL,
    chunk_id text NOT NULL,
    "order" integer DEFAULT 0 NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: user_settings; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_settings (
    id text NOT NULL,
    user_id text NOT NULL,
    key text NOT NULL,
    value jsonb NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: vocabulary_entry; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.vocabulary_entry (
    id text NOT NULL,
    word text NOT NULL,
    definition text,
    category text NOT NULL,
    expects jsonb,
    space_id text NOT NULL,
    user_id text,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: workspace; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.workspace (
    id text NOT NULL,
    name text NOT NULL,
    description text,
    user_id text NOT NULL,
    created_at timestamp without time zone DEFAULT now() NOT NULL,
    updated_at timestamp without time zone DEFAULT now() NOT NULL
);


--
-- Name: workspace_space; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.workspace_space (
    workspace_id text NOT NULL,
    space_id text NOT NULL
);


--
-- Name: activity_log activity_log_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.activity_log
    ADD CONSTRAINT activity_log_pkey PRIMARY KEY (id);


--
-- Name: behavior_cell behavior_cell_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_cell
    ADD CONSTRAINT behavior_cell_pkey PRIMARY KEY (id);


--
-- Name: behavior_cell_requirement behavior_cell_requirement_cell_id_requirement_id_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_cell_requirement
    ADD CONSTRAINT behavior_cell_requirement_cell_id_requirement_id_pk PRIMARY KEY (cell_id, requirement_id);


--
-- Name: behavior_cell behavior_cell_rule_dimension; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_cell
    ADD CONSTRAINT behavior_cell_rule_dimension UNIQUE (rule_id, dimension_id);


--
-- Name: behavior_dimension behavior_dimension_matrix_name; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_dimension
    ADD CONSTRAINT behavior_dimension_matrix_name UNIQUE (matrix_id, name);


--
-- Name: behavior_dimension behavior_dimension_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_dimension
    ADD CONSTRAINT behavior_dimension_pkey PRIMARY KEY (id);


--
-- Name: behavior_matrix behavior_matrix_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_matrix
    ADD CONSTRAINT behavior_matrix_pkey PRIMARY KEY (id);


--
-- Name: behavior_rule behavior_rule_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_rule
    ADD CONSTRAINT behavior_rule_pkey PRIMARY KEY (id);


--
-- Name: chunk_applies_to chunk_applies_to_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_applies_to
    ADD CONSTRAINT chunk_applies_to_pkey PRIMARY KEY (id);


--
-- Name: chunk_comment chunk_comment_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_comment
    ADD CONSTRAINT chunk_comment_pkey PRIMARY KEY (id);


--
-- Name: chunk_connection chunk_connection_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_connection
    ADD CONSTRAINT chunk_connection_pkey PRIMARY KEY (id);


--
-- Name: chunk_feature_delta chunk_feature_delta_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_feature_delta
    ADD CONSTRAINT chunk_feature_delta_pkey PRIMARY KEY (id);


--
-- Name: chunk_file_ref chunk_file_ref_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_file_ref
    ADD CONSTRAINT chunk_file_ref_pkey PRIMARY KEY (id);


--
-- Name: chunk chunk_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk
    ADD CONSTRAINT chunk_pkey PRIMARY KEY (id);


--
-- Name: chunk_proposal chunk_proposal_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_proposal
    ADD CONSTRAINT chunk_proposal_pkey PRIMARY KEY (id);


--
-- Name: chunk_space chunk_space_chunk_id_space_id_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_space
    ADD CONSTRAINT chunk_space_chunk_id_space_id_pk PRIMARY KEY (chunk_id, space_id);


--
-- Name: chunk_staleness chunk_staleness_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_staleness
    ADD CONSTRAINT chunk_staleness_pkey PRIMARY KEY (id);


--
-- Name: chunk_tag chunk_tag_chunk_id_tag_id_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_tag
    ADD CONSTRAINT chunk_tag_chunk_id_tag_id_pk PRIMARY KEY (chunk_id, tag_id);


--
-- Name: chunk_template chunk_template_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_template
    ADD CONSTRAINT chunk_template_pkey PRIMARY KEY (id);


--
-- Name: chunk_type chunk_type_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_type
    ADD CONSTRAINT chunk_type_pkey PRIMARY KEY (id);


--
-- Name: chunk_version chunk_version_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_version
    ADD CONSTRAINT chunk_version_pkey PRIMARY KEY (id);


--
-- Name: codebase_settings codebase_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.codebase_settings
    ADD CONSTRAINT codebase_settings_pkey PRIMARY KEY (id);


--
-- Name: collection collection_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.collection
    ADD CONSTRAINT collection_pkey PRIMARY KEY (id);


--
-- Name: connection_relation connection_relation_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.connection_relation
    ADD CONSTRAINT connection_relation_pkey PRIMARY KEY (id);


--
-- Name: context_snapshot context_snapshot_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.context_snapshot
    ADD CONSTRAINT context_snapshot_pkey PRIMARY KEY (id);


--
-- Name: document document_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.document
    ADD CONSTRAINT document_pkey PRIMARY KEY (id);


--
-- Name: feature feature_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.feature
    ADD CONSTRAINT feature_pkey PRIMARY KEY (id);


--
-- Name: feature_space feature_space_feature_id_space_id_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.feature_space
    ADD CONSTRAINT feature_space_feature_id_space_id_pk PRIMARY KEY (feature_id, space_id);


--
-- Name: instance_settings instance_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.instance_settings
    ADD CONSTRAINT instance_settings_pkey PRIMARY KEY (key);


--
-- Name: learning_path learning_path_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.learning_path
    ADD CONSTRAINT learning_path_pkey PRIMARY KEY (id);


--
-- Name: notification notification_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.notification
    ADD CONSTRAINT notification_pkey PRIMARY KEY (id);


--
-- Name: plan_analyze_item plan_analyze_item_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_analyze_item
    ADD CONSTRAINT plan_analyze_item_pkey PRIMARY KEY (id);


--
-- Name: plan_external_link plan_external_link_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_external_link
    ADD CONSTRAINT plan_external_link_pkey PRIMARY KEY (id);


--
-- Name: plan plan_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan
    ADD CONSTRAINT plan_pkey PRIMARY KEY (id);


--
-- Name: plan_requirement plan_requirement_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_requirement
    ADD CONSTRAINT plan_requirement_pkey PRIMARY KEY (id);


--
-- Name: plan_task_chunk plan_task_chunk_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_task_chunk
    ADD CONSTRAINT plan_task_chunk_pkey PRIMARY KEY (id);


--
-- Name: plan_task_dependency plan_task_dependency_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_task_dependency
    ADD CONSTRAINT plan_task_dependency_pkey PRIMARY KEY (id);


--
-- Name: plan_task_external_link plan_task_external_link_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_task_external_link
    ADD CONSTRAINT plan_task_external_link_pkey PRIMARY KEY (id);


--
-- Name: plan_task plan_task_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_task
    ADD CONSTRAINT plan_task_pkey PRIMARY KEY (id);


--
-- Name: requirement_chunk requirement_chunk_requirement_id_chunk_id_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requirement_chunk
    ADD CONSTRAINT requirement_chunk_requirement_id_chunk_id_pk PRIMARY KEY (requirement_id, chunk_id);


--
-- Name: requirement_dependency requirement_dependency_requirement_id_depends_on_id_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requirement_dependency
    ADD CONSTRAINT requirement_dependency_requirement_id_depends_on_id_pk PRIMARY KEY (requirement_id, depends_on_id);


--
-- Name: requirement requirement_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requirement
    ADD CONSTRAINT requirement_pkey PRIMARY KEY (id);


--
-- Name: saved_graph saved_graph_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.saved_graph
    ADD CONSTRAINT saved_graph_pkey PRIMARY KEY (id);


--
-- Name: saved_query saved_query_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.saved_query
    ADD CONSTRAINT saved_query_pkey PRIMARY KEY (id);


--
-- Name: scope_key scope_key_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.scope_key
    ADD CONSTRAINT scope_key_pkey PRIMARY KEY (id);


--
-- Name: scope_key scope_key_user_key_unique; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.scope_key
    ADD CONSTRAINT scope_key_user_key_unique UNIQUE (user_id, key);


--
-- Name: session session_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.session
    ADD CONSTRAINT session_pkey PRIMARY KEY (id);


--
-- Name: session session_token_unique; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.session
    ADD CONSTRAINT session_token_unique UNIQUE (token);


--
-- Name: space_code_metadata space_code_metadata_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.space_code_metadata
    ADD CONSTRAINT space_code_metadata_pkey PRIMARY KEY (space_id);


--
-- Name: space_kind space_kind_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.space_kind
    ADD CONSTRAINT space_kind_pkey PRIMARY KEY (id);


--
-- Name: space space_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.space
    ADD CONSTRAINT space_pkey PRIMARY KEY (id);


--
-- Name: staleness_scan staleness_scan_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.staleness_scan
    ADD CONSTRAINT staleness_scan_pkey PRIMARY KEY (id);


--
-- Name: tag tag_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tag
    ADD CONSTRAINT tag_pkey PRIMARY KEY (id);


--
-- Name: tag_type tag_type_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tag_type
    ADD CONSTRAINT tag_type_pkey PRIMARY KEY (id);


--
-- Name: use_case use_case_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.use_case
    ADD CONSTRAINT use_case_pkey PRIMARY KEY (id);


--
-- Name: user_active_feature user_active_feature_user_id_feature_id_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_active_feature
    ADD CONSTRAINT user_active_feature_user_id_feature_id_pk PRIMARY KEY (user_id, feature_id);


--
-- Name: user user_email_unique; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public."user"
    ADD CONSTRAINT user_email_unique UNIQUE (email);


--
-- Name: user_favorite user_favorite_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_favorite
    ADD CONSTRAINT user_favorite_pkey PRIMARY KEY (id);


--
-- Name: user user_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public."user"
    ADD CONSTRAINT user_pkey PRIMARY KEY (id);


--
-- Name: user_settings user_settings_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_settings
    ADD CONSTRAINT user_settings_pkey PRIMARY KEY (id);


--
-- Name: vocabulary_entry vocabulary_entry_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.vocabulary_entry
    ADD CONSTRAINT vocabulary_entry_pkey PRIMARY KEY (id);


--
-- Name: workspace workspace_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workspace
    ADD CONSTRAINT workspace_pkey PRIMARY KEY (id);


--
-- Name: workspace_space workspace_space_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workspace_space
    ADD CONSTRAINT workspace_space_pkey PRIMARY KEY (workspace_id, space_id);


--
-- Name: activity_codebaseId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "activity_codebaseId_idx" ON public.activity_log USING btree (space_id);


--
-- Name: activity_createdAt_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "activity_createdAt_idx" ON public.activity_log USING btree (created_at);


--
-- Name: activity_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "activity_userId_idx" ON public.activity_log USING btree (user_id);


--
-- Name: behavior_cell_dimensionId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "behavior_cell_dimensionId_idx" ON public.behavior_cell USING btree (dimension_id);


--
-- Name: behavior_cell_ruleId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "behavior_cell_ruleId_idx" ON public.behavior_cell USING btree (rule_id);


--
-- Name: behavior_dimension_matrixId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "behavior_dimension_matrixId_idx" ON public.behavior_dimension USING btree (matrix_id);


--
-- Name: behavior_matrix_layer_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX behavior_matrix_layer_idx ON public.behavior_matrix USING btree (layer);


--
-- Name: behavior_matrix_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "behavior_matrix_userId_idx" ON public.behavior_matrix USING btree (user_id);


--
-- Name: behavior_rule_matrixId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "behavior_rule_matrixId_idx" ON public.behavior_rule USING btree (matrix_id);


--
-- Name: chunk_applies_to_chunkId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_applies_to_chunkId_idx" ON public.chunk_applies_to USING btree (chunk_id);


--
-- Name: chunk_archivedAt_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_archivedAt_idx" ON public.chunk USING btree (archived_at);


--
-- Name: chunk_comment_chunkId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_comment_chunkId_idx" ON public.chunk_comment USING btree (chunk_id);


--
-- Name: chunk_comment_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_comment_userId_idx" ON public.chunk_comment USING btree (user_id);


--
-- Name: chunk_document_order_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX chunk_document_order_idx ON public.chunk USING btree (document_id, document_order) WHERE (document_id IS NOT NULL);


--
-- Name: chunk_feature_delta_chunk_feature_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX chunk_feature_delta_chunk_feature_idx ON public.chunk_feature_delta USING btree (chunk_id, feature_id);


--
-- Name: chunk_feature_delta_feature_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX chunk_feature_delta_feature_idx ON public.chunk_feature_delta USING btree (feature_id);


--
-- Name: chunk_file_ref_chunkId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_file_ref_chunkId_idx" ON public.chunk_file_ref USING btree (chunk_id);


--
-- Name: chunk_file_ref_path_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX chunk_file_ref_path_idx ON public.chunk_file_ref USING btree (path);


--
-- Name: chunk_proposal_chunkId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_proposal_chunkId_idx" ON public.chunk_proposal USING btree (chunk_id);


--
-- Name: chunk_proposal_chunkId_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_proposal_chunkId_status_idx" ON public.chunk_proposal USING btree (chunk_id, status);


--
-- Name: chunk_proposal_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX chunk_proposal_status_idx ON public.chunk_proposal USING btree (status);


--
-- Name: chunk_space_chunkId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_space_chunkId_idx" ON public.chunk_space USING btree (chunk_id);


--
-- Name: chunk_space_chunkid_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX chunk_space_chunkid_idx ON public.chunk_space USING btree (chunk_id);


--
-- Name: chunk_space_spaceId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_space_spaceId_idx" ON public.chunk_space USING btree (space_id);


--
-- Name: chunk_space_spaceid_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX chunk_space_spaceid_idx ON public.chunk_space USING btree (space_id);


--
-- Name: chunk_staleness_chunkId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_staleness_chunkId_idx" ON public.chunk_staleness USING btree (chunk_id);


--
-- Name: chunk_staleness_dismissedAt_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_staleness_dismissedAt_idx" ON public.chunk_staleness USING btree (dismissed_at);


--
-- Name: chunk_staleness_reason_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX chunk_staleness_reason_idx ON public.chunk_staleness USING btree (reason);


--
-- Name: chunk_tag_tagId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_tag_tagId_idx" ON public.chunk_tag USING btree (tag_id);


--
-- Name: chunk_type_codebaseId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_type_codebaseId_idx" ON public.chunk_type USING btree (space_id);


--
-- Name: chunk_type_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX chunk_type_idx ON public.chunk USING btree (type);


--
-- Name: chunk_type_scope_id_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX chunk_type_scope_id_idx ON public.chunk_type USING btree (id);


--
-- Name: chunk_type_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_type_userId_idx" ON public.chunk_type USING btree (user_id);


--
-- Name: chunk_updatedAt_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_updatedAt_idx" ON public.chunk USING btree (updated_at);


--
-- Name: chunk_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_userId_idx" ON public.chunk USING btree (user_id);


--
-- Name: chunk_version_chunkId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "chunk_version_chunkId_idx" ON public.chunk_version USING btree (chunk_id);


--
-- Name: chunk_version_update_tag_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX chunk_version_update_tag_idx ON public.chunk_version USING btree (update_tag);


--
-- Name: codebase_settings_cb_key_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX codebase_settings_cb_key_idx ON public.codebase_settings USING btree (space_id, key);


--
-- Name: codebase_settings_codebaseId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "codebase_settings_codebaseId_idx" ON public.codebase_settings USING btree (space_id);


--
-- Name: collection_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "collection_userId_idx" ON public.collection USING btree (user_id);


--
-- Name: collection_user_name_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX collection_user_name_idx ON public.collection USING btree (user_id, name);


--
-- Name: connection_relation_codebaseId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "connection_relation_codebaseId_idx" ON public.connection_relation USING btree (space_id);


--
-- Name: connection_relation_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "connection_relation_userId_idx" ON public.connection_relation USING btree (user_id);


--
-- Name: connection_sourceId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "connection_sourceId_idx" ON public.chunk_connection USING btree (source_id);


--
-- Name: connection_targetId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "connection_targetId_idx" ON public.chunk_connection USING btree (target_id);


--
-- Name: connection_unique_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX connection_unique_idx ON public.chunk_connection USING btree (source_id, target_id, relation);


--
-- Name: document_codebaseId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "document_codebaseId_idx" ON public.document USING btree (space_id);


--
-- Name: document_source_codebase_user_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX document_source_codebase_user_idx ON public.document USING btree (source_path, space_id, user_id);


--
-- Name: document_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "document_userId_idx" ON public.document USING btree (user_id);


--
-- Name: favorite_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "favorite_userId_idx" ON public.user_favorite USING btree (user_id);


--
-- Name: favorite_user_chunk_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX favorite_user_chunk_idx ON public.user_favorite USING btree (user_id, chunk_id);


--
-- Name: feature_user_name_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX feature_user_name_idx ON public.feature USING btree (user_id, name);


--
-- Name: feature_user_priority_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX feature_user_priority_idx ON public.feature USING btree (user_id, priority);


--
-- Name: learning_path_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "learning_path_userId_idx" ON public.learning_path USING btree (user_id);


--
-- Name: notification_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "notification_userId_idx" ON public.notification USING btree (user_id);


--
-- Name: notification_userId_read_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "notification_userId_read_idx" ON public.notification USING btree (user_id, read);


--
-- Name: plan_analyze_item_chunkId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "plan_analyze_item_chunkId_idx" ON public.plan_analyze_item USING btree (chunk_id);


--
-- Name: plan_analyze_item_planId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "plan_analyze_item_planId_idx" ON public.plan_analyze_item USING btree (plan_id);


--
-- Name: plan_codebaseId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "plan_codebaseId_idx" ON public.plan USING btree (space_id);


--
-- Name: plan_external_link_planId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "plan_external_link_planId_idx" ON public.plan_external_link USING btree (plan_id);


--
-- Name: plan_requirement_planId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "plan_requirement_planId_idx" ON public.plan_requirement USING btree (plan_id);


--
-- Name: plan_requirement_unique_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX plan_requirement_unique_idx ON public.plan_requirement USING btree (plan_id, requirement_id);


--
-- Name: plan_task_chunk_taskId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "plan_task_chunk_taskId_idx" ON public.plan_task_chunk USING btree (task_id);


--
-- Name: plan_task_chunk_unique_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX plan_task_chunk_unique_idx ON public.plan_task_chunk USING btree (task_id, chunk_id, relation);


--
-- Name: plan_task_dependency_taskId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "plan_task_dependency_taskId_idx" ON public.plan_task_dependency USING btree (task_id);


--
-- Name: plan_task_dependency_unique_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX plan_task_dependency_unique_idx ON public.plan_task_dependency USING btree (task_id, depends_on_task_id);


--
-- Name: plan_task_external_link_taskId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "plan_task_external_link_taskId_idx" ON public.plan_task_external_link USING btree (task_id);


--
-- Name: plan_task_planId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "plan_task_planId_idx" ON public.plan_task USING btree (plan_id);


--
-- Name: plan_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "plan_userId_idx" ON public.plan USING btree (user_id);


--
-- Name: requirement_codebaseId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "requirement_codebaseId_idx" ON public.requirement USING btree (space_id);


--
-- Name: requirement_status_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX requirement_status_idx ON public.requirement USING btree (status);


--
-- Name: requirement_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "requirement_userId_idx" ON public.requirement USING btree (user_id);


--
-- Name: saved_graph_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "saved_graph_userId_idx" ON public.saved_graph USING btree (user_id);


--
-- Name: saved_query_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "saved_query_userId_idx" ON public.saved_query USING btree (user_id);


--
-- Name: session_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "session_userId_idx" ON public.session USING btree (user_id);


--
-- Name: space_code_user_remote_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX space_code_user_remote_idx ON public.space_code_metadata USING btree (user_id, remote_url) WHERE (remote_url IS NOT NULL);


--
-- Name: space_kind_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX space_kind_idx ON public.space USING btree (kind);


--
-- Name: space_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "space_userId_idx" ON public.space USING btree (user_id);


--
-- Name: space_user_name_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX space_user_name_idx ON public.space USING btree (user_id, name);


--
-- Name: space_userid_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX space_userid_idx ON public.space USING btree (user_id);


--
-- Name: staleness_scan_codebaseId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "staleness_scan_codebaseId_idx" ON public.staleness_scan USING btree (space_id);


--
-- Name: tag_type_user_name_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX tag_type_user_name_idx ON public.tag_type USING btree (user_id, name);


--
-- Name: tag_user_name_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX tag_user_name_idx ON public.tag USING btree (user_id, name);


--
-- Name: template_builtin_name_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX template_builtin_name_idx ON public.chunk_template USING btree (name) WHERE (user_id IS NULL);


--
-- Name: template_user_name_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX template_user_name_idx ON public.chunk_template USING btree (user_id, name) WHERE (user_id IS NOT NULL);


--
-- Name: use_case_parentId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "use_case_parentId_idx" ON public.use_case USING btree (parent_id);


--
-- Name: use_case_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "use_case_userId_idx" ON public.use_case USING btree (user_id);


--
-- Name: use_case_user_name_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX use_case_user_name_idx ON public.use_case USING btree (user_id, name);


--
-- Name: user_settings_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "user_settings_userId_idx" ON public.user_settings USING btree (user_id);


--
-- Name: user_settings_user_key_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX user_settings_user_key_idx ON public.user_settings USING btree (user_id, key);


--
-- Name: vocabulary_codebaseId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "vocabulary_codebaseId_idx" ON public.vocabulary_entry USING btree (space_id);


--
-- Name: vocabulary_codebase_word_cat_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX vocabulary_codebase_word_cat_idx ON public.vocabulary_entry USING btree (space_id, category, lower(word));


--
-- Name: workspace_space_spaceid_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX workspace_space_spaceid_idx ON public.workspace_space USING btree (space_id);


--
-- Name: workspace_space_workspaceid_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX workspace_space_workspaceid_idx ON public.workspace_space USING btree (workspace_id);


--
-- Name: workspace_userId_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX "workspace_userId_idx" ON public.workspace USING btree (user_id);


--
-- Name: workspace_user_name_idx; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX workspace_user_name_idx ON public.workspace USING btree (user_id, name);


--
-- Name: activity_log activity_log_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.activity_log
    ADD CONSTRAINT activity_log_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE SET NULL;


--
-- Name: activity_log activity_log_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.activity_log
    ADD CONSTRAINT activity_log_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: behavior_cell behavior_cell_dimension_id_behavior_dimension_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_cell
    ADD CONSTRAINT behavior_cell_dimension_id_behavior_dimension_id_fk FOREIGN KEY (dimension_id) REFERENCES public.behavior_dimension(id) ON DELETE CASCADE;


--
-- Name: behavior_cell_requirement behavior_cell_requirement_cell_id_behavior_cell_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_cell_requirement
    ADD CONSTRAINT behavior_cell_requirement_cell_id_behavior_cell_id_fk FOREIGN KEY (cell_id) REFERENCES public.behavior_cell(id) ON DELETE CASCADE;


--
-- Name: behavior_cell_requirement behavior_cell_requirement_requirement_id_requirement_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_cell_requirement
    ADD CONSTRAINT behavior_cell_requirement_requirement_id_requirement_id_fk FOREIGN KEY (requirement_id) REFERENCES public.requirement(id) ON DELETE CASCADE;


--
-- Name: behavior_cell behavior_cell_rule_id_behavior_rule_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_cell
    ADD CONSTRAINT behavior_cell_rule_id_behavior_rule_id_fk FOREIGN KEY (rule_id) REFERENCES public.behavior_rule(id) ON DELETE CASCADE;


--
-- Name: behavior_dimension behavior_dimension_matrix_id_behavior_matrix_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_dimension
    ADD CONSTRAINT behavior_dimension_matrix_id_behavior_matrix_id_fk FOREIGN KEY (matrix_id) REFERENCES public.behavior_matrix(id) ON DELETE CASCADE;


--
-- Name: behavior_matrix behavior_matrix_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_matrix
    ADD CONSTRAINT behavior_matrix_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE SET NULL;


--
-- Name: behavior_matrix behavior_matrix_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_matrix
    ADD CONSTRAINT behavior_matrix_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: behavior_rule behavior_rule_matrix_id_behavior_matrix_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.behavior_rule
    ADD CONSTRAINT behavior_rule_matrix_id_behavior_matrix_id_fk FOREIGN KEY (matrix_id) REFERENCES public.behavior_matrix(id) ON DELETE CASCADE;


--
-- Name: chunk_applies_to chunk_applies_to_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_applies_to
    ADD CONSTRAINT chunk_applies_to_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: chunk_comment chunk_comment_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_comment
    ADD CONSTRAINT chunk_comment_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: chunk_comment chunk_comment_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_comment
    ADD CONSTRAINT chunk_comment_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: chunk_connection chunk_connection_relation_connection_relation_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_connection
    ADD CONSTRAINT chunk_connection_relation_connection_relation_id_fk FOREIGN KEY (relation) REFERENCES public.connection_relation(id) ON DELETE RESTRICT;


--
-- Name: chunk_connection chunk_connection_reviewed_by_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_connection
    ADD CONSTRAINT chunk_connection_reviewed_by_user_id_fk FOREIGN KEY (reviewed_by) REFERENCES public."user"(id) ON DELETE SET NULL;


--
-- Name: chunk_connection chunk_connection_source_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_connection
    ADD CONSTRAINT chunk_connection_source_id_chunk_id_fk FOREIGN KEY (source_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: chunk_connection chunk_connection_target_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_connection
    ADD CONSTRAINT chunk_connection_target_id_chunk_id_fk FOREIGN KEY (target_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: chunk chunk_document_id_document_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk
    ADD CONSTRAINT chunk_document_id_document_id_fk FOREIGN KEY (document_id) REFERENCES public.document(id) ON DELETE SET NULL;


--
-- Name: chunk_feature_delta chunk_feature_delta_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_feature_delta
    ADD CONSTRAINT chunk_feature_delta_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: chunk_feature_delta chunk_feature_delta_feature_id_feature_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_feature_delta
    ADD CONSTRAINT chunk_feature_delta_feature_id_feature_id_fk FOREIGN KEY (feature_id) REFERENCES public.feature(id) ON DELETE CASCADE;


--
-- Name: chunk_file_ref chunk_file_ref_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_file_ref
    ADD CONSTRAINT chunk_file_ref_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: chunk_proposal chunk_proposal_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_proposal
    ADD CONSTRAINT chunk_proposal_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: chunk_proposal chunk_proposal_reviewed_by_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_proposal
    ADD CONSTRAINT chunk_proposal_reviewed_by_user_id_fk FOREIGN KEY (reviewed_by) REFERENCES public."user"(id) ON DELETE SET NULL;


--
-- Name: chunk chunk_reviewed_by_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk
    ADD CONSTRAINT chunk_reviewed_by_user_id_fk FOREIGN KEY (reviewed_by) REFERENCES public."user"(id) ON DELETE SET NULL;


--
-- Name: chunk_space chunk_space_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_space
    ADD CONSTRAINT chunk_space_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: chunk_space chunk_space_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_space
    ADD CONSTRAINT chunk_space_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE CASCADE;


--
-- Name: chunk_staleness chunk_staleness_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_staleness
    ADD CONSTRAINT chunk_staleness_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: chunk_staleness chunk_staleness_dismissed_by_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_staleness
    ADD CONSTRAINT chunk_staleness_dismissed_by_user_id_fk FOREIGN KEY (dismissed_by) REFERENCES public."user"(id) ON DELETE SET NULL;


--
-- Name: chunk_staleness chunk_staleness_related_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_staleness
    ADD CONSTRAINT chunk_staleness_related_chunk_id_chunk_id_fk FOREIGN KEY (related_chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: chunk_tag chunk_tag_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_tag
    ADD CONSTRAINT chunk_tag_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: chunk_tag chunk_tag_tag_id_tag_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_tag
    ADD CONSTRAINT chunk_tag_tag_id_tag_id_fk FOREIGN KEY (tag_id) REFERENCES public.tag(id) ON DELETE CASCADE;


--
-- Name: chunk_template chunk_template_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_template
    ADD CONSTRAINT chunk_template_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: chunk chunk_type_chunk_type_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk
    ADD CONSTRAINT chunk_type_chunk_type_id_fk FOREIGN KEY (type) REFERENCES public.chunk_type(id) ON DELETE RESTRICT;


--
-- Name: chunk_type chunk_type_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_type
    ADD CONSTRAINT chunk_type_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE CASCADE;


--
-- Name: chunk_type chunk_type_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_type
    ADD CONSTRAINT chunk_type_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: chunk chunk_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk
    ADD CONSTRAINT chunk_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: chunk_version chunk_version_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.chunk_version
    ADD CONSTRAINT chunk_version_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: codebase_settings codebase_settings_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.codebase_settings
    ADD CONSTRAINT codebase_settings_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE CASCADE;


--
-- Name: collection collection_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.collection
    ADD CONSTRAINT collection_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE SET NULL;


--
-- Name: collection collection_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.collection
    ADD CONSTRAINT collection_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: connection_relation connection_relation_inverse_of_id_connection_relation_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.connection_relation
    ADD CONSTRAINT connection_relation_inverse_of_id_connection_relation_id_fk FOREIGN KEY (inverse_of_id) REFERENCES public.connection_relation(id) ON DELETE SET NULL;


--
-- Name: connection_relation connection_relation_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.connection_relation
    ADD CONSTRAINT connection_relation_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE CASCADE;


--
-- Name: connection_relation connection_relation_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.connection_relation
    ADD CONSTRAINT connection_relation_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: context_snapshot context_snapshot_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.context_snapshot
    ADD CONSTRAINT context_snapshot_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: document document_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.document
    ADD CONSTRAINT document_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE SET NULL;


--
-- Name: document document_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.document
    ADD CONSTRAINT document_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: feature_space feature_space_feature_id_feature_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.feature_space
    ADD CONSTRAINT feature_space_feature_id_feature_id_fk FOREIGN KEY (feature_id) REFERENCES public.feature(id) ON DELETE CASCADE;


--
-- Name: feature_space feature_space_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.feature_space
    ADD CONSTRAINT feature_space_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE CASCADE;


--
-- Name: feature feature_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.feature
    ADD CONSTRAINT feature_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: learning_path learning_path_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.learning_path
    ADD CONSTRAINT learning_path_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: notification notification_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.notification
    ADD CONSTRAINT notification_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: plan_analyze_item plan_analyze_item_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_analyze_item
    ADD CONSTRAINT plan_analyze_item_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: plan_analyze_item plan_analyze_item_plan_id_plan_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_analyze_item
    ADD CONSTRAINT plan_analyze_item_plan_id_plan_id_fk FOREIGN KEY (plan_id) REFERENCES public.plan(id) ON DELETE CASCADE;


--
-- Name: plan_external_link plan_external_link_plan_id_plan_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_external_link
    ADD CONSTRAINT plan_external_link_plan_id_plan_id_fk FOREIGN KEY (plan_id) REFERENCES public.plan(id) ON DELETE CASCADE;


--
-- Name: plan_requirement plan_requirement_plan_id_plan_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_requirement
    ADD CONSTRAINT plan_requirement_plan_id_plan_id_fk FOREIGN KEY (plan_id) REFERENCES public.plan(id) ON DELETE CASCADE;


--
-- Name: plan_requirement plan_requirement_requirement_id_requirement_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_requirement
    ADD CONSTRAINT plan_requirement_requirement_id_requirement_id_fk FOREIGN KEY (requirement_id) REFERENCES public.requirement(id) ON DELETE CASCADE;


--
-- Name: plan plan_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan
    ADD CONSTRAINT plan_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE SET NULL;


--
-- Name: plan_task_chunk plan_task_chunk_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_task_chunk
    ADD CONSTRAINT plan_task_chunk_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: plan_task_chunk plan_task_chunk_task_id_plan_task_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_task_chunk
    ADD CONSTRAINT plan_task_chunk_task_id_plan_task_id_fk FOREIGN KEY (task_id) REFERENCES public.plan_task(id) ON DELETE CASCADE;


--
-- Name: plan_task_dependency plan_task_dependency_depends_on_task_id_plan_task_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_task_dependency
    ADD CONSTRAINT plan_task_dependency_depends_on_task_id_plan_task_id_fk FOREIGN KEY (depends_on_task_id) REFERENCES public.plan_task(id) ON DELETE CASCADE;


--
-- Name: plan_task_dependency plan_task_dependency_task_id_plan_task_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_task_dependency
    ADD CONSTRAINT plan_task_dependency_task_id_plan_task_id_fk FOREIGN KEY (task_id) REFERENCES public.plan_task(id) ON DELETE CASCADE;


--
-- Name: plan_task_external_link plan_task_external_link_task_id_plan_task_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_task_external_link
    ADD CONSTRAINT plan_task_external_link_task_id_plan_task_id_fk FOREIGN KEY (task_id) REFERENCES public.plan_task(id) ON DELETE CASCADE;


--
-- Name: plan_task plan_task_plan_id_plan_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan_task
    ADD CONSTRAINT plan_task_plan_id_plan_id_fk FOREIGN KEY (plan_id) REFERENCES public.plan(id) ON DELETE CASCADE;


--
-- Name: plan plan_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.plan
    ADD CONSTRAINT plan_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: requirement_chunk requirement_chunk_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requirement_chunk
    ADD CONSTRAINT requirement_chunk_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: requirement_chunk requirement_chunk_requirement_id_requirement_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requirement_chunk
    ADD CONSTRAINT requirement_chunk_requirement_id_requirement_id_fk FOREIGN KEY (requirement_id) REFERENCES public.requirement(id) ON DELETE CASCADE;


--
-- Name: requirement_dependency requirement_dependency_depends_on_id_requirement_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requirement_dependency
    ADD CONSTRAINT requirement_dependency_depends_on_id_requirement_id_fk FOREIGN KEY (depends_on_id) REFERENCES public.requirement(id) ON DELETE CASCADE;


--
-- Name: requirement_dependency requirement_dependency_requirement_id_requirement_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requirement_dependency
    ADD CONSTRAINT requirement_dependency_requirement_id_requirement_id_fk FOREIGN KEY (requirement_id) REFERENCES public.requirement(id) ON DELETE CASCADE;


--
-- Name: requirement requirement_reviewed_by_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requirement
    ADD CONSTRAINT requirement_reviewed_by_user_id_fk FOREIGN KEY (reviewed_by) REFERENCES public."user"(id) ON DELETE SET NULL;


--
-- Name: requirement requirement_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requirement
    ADD CONSTRAINT requirement_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE SET NULL;


--
-- Name: requirement requirement_use_case_id_use_case_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requirement
    ADD CONSTRAINT requirement_use_case_id_use_case_id_fk FOREIGN KEY (use_case_id) REFERENCES public.use_case(id) ON DELETE SET NULL;


--
-- Name: requirement requirement_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.requirement
    ADD CONSTRAINT requirement_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: saved_graph saved_graph_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.saved_graph
    ADD CONSTRAINT saved_graph_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE SET NULL;


--
-- Name: saved_graph saved_graph_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.saved_graph
    ADD CONSTRAINT saved_graph_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: saved_query saved_query_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.saved_query
    ADD CONSTRAINT saved_query_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE SET NULL;


--
-- Name: saved_query saved_query_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.saved_query
    ADD CONSTRAINT saved_query_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: scope_key scope_key_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.scope_key
    ADD CONSTRAINT scope_key_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: session session_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.session
    ADD CONSTRAINT session_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: space_code_metadata space_code_metadata_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.space_code_metadata
    ADD CONSTRAINT space_code_metadata_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE CASCADE;


--
-- Name: space space_kind_space_kind_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.space
    ADD CONSTRAINT space_kind_space_kind_id_fk FOREIGN KEY (kind) REFERENCES public.space_kind(id) ON DELETE RESTRICT;


--
-- Name: space space_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.space
    ADD CONSTRAINT space_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: staleness_scan staleness_scan_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.staleness_scan
    ADD CONSTRAINT staleness_scan_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE CASCADE;


--
-- Name: tag tag_reviewed_by_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tag
    ADD CONSTRAINT tag_reviewed_by_user_id_fk FOREIGN KEY (reviewed_by) REFERENCES public."user"(id) ON DELETE SET NULL;


--
-- Name: tag tag_tag_type_id_tag_type_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tag
    ADD CONSTRAINT tag_tag_type_id_tag_type_id_fk FOREIGN KEY (tag_type_id) REFERENCES public.tag_type(id) ON DELETE SET NULL;


--
-- Name: tag_type tag_type_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tag_type
    ADD CONSTRAINT tag_type_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: tag tag_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.tag
    ADD CONSTRAINT tag_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: use_case use_case_parent_id_use_case_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.use_case
    ADD CONSTRAINT use_case_parent_id_use_case_id_fk FOREIGN KEY (parent_id) REFERENCES public.use_case(id) ON DELETE CASCADE;


--
-- Name: use_case use_case_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.use_case
    ADD CONSTRAINT use_case_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE SET NULL;


--
-- Name: use_case use_case_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.use_case
    ADD CONSTRAINT use_case_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: user_active_feature user_active_feature_feature_id_feature_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_active_feature
    ADD CONSTRAINT user_active_feature_feature_id_feature_id_fk FOREIGN KEY (feature_id) REFERENCES public.feature(id) ON DELETE CASCADE;


--
-- Name: user_active_feature user_active_feature_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_active_feature
    ADD CONSTRAINT user_active_feature_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: user_favorite user_favorite_chunk_id_chunk_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_favorite
    ADD CONSTRAINT user_favorite_chunk_id_chunk_id_fk FOREIGN KEY (chunk_id) REFERENCES public.chunk(id) ON DELETE CASCADE;


--
-- Name: user_favorite user_favorite_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_favorite
    ADD CONSTRAINT user_favorite_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: user_settings user_settings_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_settings
    ADD CONSTRAINT user_settings_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- Name: vocabulary_entry vocabulary_entry_space_id_space_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.vocabulary_entry
    ADD CONSTRAINT vocabulary_entry_space_id_space_id_fk FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE CASCADE;


--
-- Name: vocabulary_entry vocabulary_entry_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.vocabulary_entry
    ADD CONSTRAINT vocabulary_entry_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE SET NULL;


--
-- Name: workspace_space workspace_space_space_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workspace_space
    ADD CONSTRAINT workspace_space_space_id_fkey FOREIGN KEY (space_id) REFERENCES public.space(id) ON DELETE CASCADE;


--
-- Name: workspace_space workspace_space_workspace_id_fkey; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workspace_space
    ADD CONSTRAINT workspace_space_workspace_id_fkey FOREIGN KEY (workspace_id) REFERENCES public.workspace(id) ON DELETE CASCADE;


--
-- Name: workspace workspace_user_id_user_id_fk; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.workspace
    ADD CONSTRAINT workspace_user_id_user_id_fk FOREIGN KEY (user_id) REFERENCES public."user"(id) ON DELETE CASCADE;


--
-- PostgreSQL database dump complete
--

