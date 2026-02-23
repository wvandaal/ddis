package storage

// SchemaSQL contains all CREATE TABLE statements for the DDIS spec index.
const SchemaSQL = `
-- The spec itself (one row per parsed spec)
CREATE TABLE IF NOT EXISTS spec_index (
    id INTEGER PRIMARY KEY,
    spec_path TEXT NOT NULL,
    spec_name TEXT,
    ddis_version TEXT,
    total_lines INTEGER,
    content_hash TEXT NOT NULL,
    parsed_at TEXT NOT NULL,
    source_type TEXT NOT NULL
      CHECK(source_type IN ('monolith', 'modular'))
);

-- Source files (1 for monolith, N for modular)
CREATE TABLE IF NOT EXISTS source_files (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    file_path TEXT NOT NULL,
    file_role TEXT NOT NULL
      CHECK(file_role IN (
        'monolith', 'manifest', 'system_constitution',
        'domain_constitution', 'deep_context', 'module'
      )),
    module_name TEXT,
    content_hash TEXT NOT NULL,
    line_count INTEGER NOT NULL,
    raw_text TEXT NOT NULL,
    UNIQUE(spec_id, file_path)
);

-- Hierarchical sections (heading tree)
CREATE TABLE IF NOT EXISTS sections (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    source_file_id INTEGER NOT NULL REFERENCES source_files(id),
    section_path TEXT NOT NULL,
    title TEXT NOT NULL,
    heading_level INTEGER NOT NULL,
    parent_id INTEGER REFERENCES sections(id),
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    raw_text TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    UNIQUE(spec_id, source_file_id, section_path)
);
CREATE INDEX IF NOT EXISTS idx_sections_path ON sections(spec_id, section_path);

-- Invariant blocks
CREATE TABLE IF NOT EXISTS invariants (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    source_file_id INTEGER NOT NULL REFERENCES source_files(id),
    section_id INTEGER NOT NULL REFERENCES sections(id),
    invariant_id TEXT NOT NULL,
    title TEXT NOT NULL,
    statement TEXT NOT NULL,
    semi_formal TEXT,
    violation_scenario TEXT,
    validation_method TEXT,
    why_this_matters TEXT,
    conditional_tag TEXT,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    raw_text TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    UNIQUE(spec_id, invariant_id)
);
CREATE INDEX IF NOT EXISTS idx_inv_id ON invariants(spec_id, invariant_id);

-- ADR blocks
CREATE TABLE IF NOT EXISTS adrs (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    source_file_id INTEGER NOT NULL REFERENCES source_files(id),
    section_id INTEGER NOT NULL REFERENCES sections(id),
    adr_id TEXT NOT NULL,
    title TEXT NOT NULL,
    problem TEXT NOT NULL,
    decision_text TEXT NOT NULL,
    chosen_option TEXT,
    consequences TEXT,
    tests TEXT,
    confidence TEXT
      CHECK(confidence IN ('Committed', 'Provisional', 'Speculative', NULL)),
    status TEXT DEFAULT 'active'
      CHECK(status IN ('active', 'superseded')),
    superseded_by TEXT,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    raw_text TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    UNIQUE(spec_id, adr_id)
);

-- ADR options (normalized)
CREATE TABLE IF NOT EXISTS adr_options (
    id INTEGER PRIMARY KEY,
    adr_id INTEGER NOT NULL REFERENCES adrs(id),
    option_label TEXT NOT NULL,
    option_name TEXT NOT NULL,
    pros TEXT,
    cons TEXT,
    is_chosen INTEGER NOT NULL DEFAULT 0,
    why_not TEXT
);

-- Quality gates
CREATE TABLE IF NOT EXISTS quality_gates (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    section_id INTEGER NOT NULL REFERENCES sections(id),
    gate_id TEXT NOT NULL,
    title TEXT NOT NULL,
    predicate TEXT NOT NULL,
    is_modular INTEGER NOT NULL DEFAULT 0,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    raw_text TEXT NOT NULL,
    UNIQUE(spec_id, gate_id)
);

-- Negative specifications
CREATE TABLE IF NOT EXISTS negative_specs (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    source_file_id INTEGER NOT NULL REFERENCES source_files(id),
    section_id INTEGER NOT NULL REFERENCES sections(id),
    constraint_text TEXT NOT NULL,
    reason TEXT,
    invariant_ref TEXT,
    line_number INTEGER NOT NULL,
    raw_text TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_neg_section ON negative_specs(section_id);

-- Verification prompt blocks
CREATE TABLE IF NOT EXISTS verification_prompts (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    section_id INTEGER NOT NULL REFERENCES sections(id),
    chapter_name TEXT NOT NULL,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    raw_text TEXT NOT NULL
);

-- Individual verification checks
CREATE TABLE IF NOT EXISTS verification_checks (
    id INTEGER PRIMARY KEY,
    prompt_id INTEGER NOT NULL REFERENCES verification_prompts(id),
    check_type TEXT NOT NULL
      CHECK(check_type IN ('positive', 'negative', 'integration')),
    check_text TEXT NOT NULL,
    invariant_ref TEXT,
    ordinal INTEGER NOT NULL
);

-- Meta-instruction blocks
CREATE TABLE IF NOT EXISTS meta_instructions (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    section_id INTEGER NOT NULL REFERENCES sections(id),
    directive TEXT NOT NULL,
    reason TEXT,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    raw_text TEXT NOT NULL
);

-- Worked example blocks
CREATE TABLE IF NOT EXISTS worked_examples (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    section_id INTEGER NOT NULL REFERENCES sections(id),
    title TEXT,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    raw_text TEXT NOT NULL
);

-- WHY NOT annotations
CREATE TABLE IF NOT EXISTS why_not_annotations (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    section_id INTEGER NOT NULL REFERENCES sections(id),
    alternative TEXT NOT NULL,
    explanation TEXT NOT NULL,
    adr_ref TEXT,
    line_number INTEGER NOT NULL,
    raw_text TEXT NOT NULL
);

-- Comparison blocks
CREATE TABLE IF NOT EXISTS comparison_blocks (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    section_id INTEGER NOT NULL REFERENCES sections(id),
    suboptimal_approach TEXT NOT NULL,
    chosen_approach TEXT NOT NULL,
    suboptimal_reasons TEXT,
    chosen_reasons TEXT,
    adr_ref TEXT,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    raw_text TEXT NOT NULL
);

-- Performance budgets
CREATE TABLE IF NOT EXISTS performance_budgets (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    section_id INTEGER NOT NULL REFERENCES sections(id),
    design_point TEXT,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    raw_text TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS budget_entries (
    id INTEGER PRIMARY KEY,
    budget_id INTEGER NOT NULL REFERENCES performance_budgets(id),
    metric_id TEXT,
    operation TEXT NOT NULL,
    target TEXT NOT NULL,
    measurement_method TEXT,
    ordinal INTEGER NOT NULL
);

-- State machines
CREATE TABLE IF NOT EXISTS state_machines (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    section_id INTEGER NOT NULL REFERENCES sections(id),
    title TEXT,
    line_start INTEGER NOT NULL,
    line_end INTEGER NOT NULL,
    raw_text TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS state_machine_cells (
    id INTEGER PRIMARY KEY,
    machine_id INTEGER NOT NULL REFERENCES state_machines(id),
    state_name TEXT NOT NULL,
    event_name TEXT NOT NULL,
    transition TEXT NOT NULL,
    guard TEXT,
    is_invalid INTEGER NOT NULL DEFAULT 0
);

-- Glossary entries
CREATE TABLE IF NOT EXISTS glossary_entries (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    section_id INTEGER NOT NULL REFERENCES sections(id),
    term TEXT NOT NULL,
    definition TEXT NOT NULL,
    section_ref TEXT,
    line_number INTEGER NOT NULL,
    UNIQUE(spec_id, term)
);
CREATE INDEX IF NOT EXISTS idx_glossary ON glossary_entries(spec_id, term);

-- Cross-reference graph
CREATE TABLE IF NOT EXISTS cross_references (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    source_file_id INTEGER NOT NULL REFERENCES source_files(id),
    source_section_id INTEGER REFERENCES sections(id),
    source_line INTEGER NOT NULL,
    ref_type TEXT NOT NULL
      CHECK(ref_type IN (
        'section', 'invariant', 'adr', 'gate',
        'app_invariant', 'app_adr', 'glossary_term'
      )),
    ref_target TEXT NOT NULL,
    ref_text TEXT NOT NULL,
    resolved INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_xref_target ON cross_references(spec_id, ref_target);
CREATE INDEX IF NOT EXISTS idx_xref_source ON cross_references(spec_id, source_section_id);

-- Modular structure
CREATE TABLE IF NOT EXISTS modules (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    source_file_id INTEGER NOT NULL REFERENCES source_files(id),
    module_name TEXT NOT NULL,
    domain TEXT NOT NULL,
    deep_context_path TEXT,
    line_count INTEGER NOT NULL,
    UNIQUE(spec_id, module_name)
);

CREATE TABLE IF NOT EXISTS module_relationships (
    id INTEGER PRIMARY KEY,
    module_id INTEGER NOT NULL REFERENCES modules(id),
    rel_type TEXT NOT NULL
      CHECK(rel_type IN ('maintains', 'interfaces', 'implements', 'adjacent')),
    target TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS module_negative_specs (
    id INTEGER PRIMARY KEY,
    module_id INTEGER NOT NULL REFERENCES modules(id),
    constraint_text TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS manifest (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    ddis_version TEXT,
    spec_name TEXT,
    tier_mode TEXT,
    target_lines INTEGER,
    hard_ceiling_lines INTEGER,
    reasoning_reserve REAL,
    raw_yaml TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS invariant_registry (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    invariant_id TEXT NOT NULL,
    owner TEXT NOT NULL,
    domain TEXT NOT NULL,
    description TEXT NOT NULL,
    UNIQUE(spec_id, invariant_id)
);

-- Transaction system (schema created now, used in Phase 3)
CREATE TABLE IF NOT EXISTS transactions (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    tx_id TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL,
    status TEXT NOT NULL
      CHECK(status IN ('pending', 'committed', 'rolled_back')),
    created_at TEXT NOT NULL,
    committed_at TEXT,
    parent_tx_id TEXT REFERENCES transactions(tx_id)
);

CREATE TABLE IF NOT EXISTS tx_operations (
    id INTEGER PRIMARY KEY,
    tx_id TEXT NOT NULL REFERENCES transactions(tx_id),
    ordinal INTEGER NOT NULL,
    operation_type TEXT NOT NULL,
    operation_data TEXT NOT NULL,
    impact_set TEXT,
    applied_at TEXT
);

-- Formatting preservation
CREATE TABLE IF NOT EXISTS formatting_hints (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    source_file_id INTEGER NOT NULL REFERENCES source_files(id),
    line_number INTEGER NOT NULL,
    hint_type TEXT NOT NULL,
    hint_value TEXT
);

-- Full-text search index (FTS5)
CREATE VIRTUAL TABLE IF NOT EXISTS fts_index USING fts5(
    element_type,
    element_id,
    title,
    content,
    content=''
);

-- LSI vectors stored as blobs (k floats per document)
CREATE TABLE IF NOT EXISTS search_vectors (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    element_type TEXT NOT NULL,
    element_id TEXT NOT NULL,
    vector BLOB NOT NULL,
    UNIQUE(spec_id, element_type, element_id)
);

-- Search model metadata
CREATE TABLE IF NOT EXISTS search_model (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    model_type TEXT NOT NULL,
    k_dimensions INTEGER NOT NULL,
    term_count INTEGER NOT NULL,
    doc_count INTEGER NOT NULL,
    built_at TEXT NOT NULL,
    model_data BLOB NOT NULL
);

-- PageRank authority scores
CREATE TABLE IF NOT EXISTS search_authority (
    id INTEGER PRIMARY KEY,
    spec_id INTEGER NOT NULL REFERENCES spec_index(id),
    element_id TEXT NOT NULL,
    score REAL NOT NULL,
    UNIQUE(spec_id, element_id)
);
`
