package tests

// Synthetic test fixtures for L1 (default) tests.
// All data is inserted via storage APIs — no file I/O, no parser dependency.
// Follows the pattern established in internal/checklist/checklist_test.go.

import (
	"database/sql"
	"os"
	"testing"

	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

// buildSyntheticDB creates an in-memory DB with a minimal but complete
// DDIS spec for fast, deterministic tests. No file I/O.
// Returns the raw *sql.DB and the spec ID.
func buildSyntheticDB(t *testing.T) (*sql.DB, int64) {
	t.Helper()

	dbPath := ""
	if os.Getenv("DDIS_TEST_DB_FILE") != "" {
		dbPath = t.TempDir() + "/synthetic.db"
	}
	if dbPath == "" {
		dbPath = t.TempDir() + "/synthetic.db"
	}

	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	// --- spec_index ---
	specID, err := storage.InsertSpecIndex(db, &storage.SpecIndex{
		SpecPath:    "/test/synthetic-spec.md",
		SpecName:    "Synthetic Test Spec",
		DDISVersion: "3.0",
		TotalLines:  500,
		ContentHash: "synth-abc123",
		SourceType:  "monolith",
	})
	if err != nil {
		t.Fatalf("insert spec: %v", err)
	}

	// --- source_files ---
	sfID, err := storage.InsertSourceFile(db, &storage.SourceFile{
		SpecID:      specID,
		FilePath:    "/test/synthetic-spec.md",
		FileRole:    "monolith",
		ContentHash: "synth-abc123",
		LineCount:   500,
		RawText:     "# Synthetic Test Spec\n\nThis is a synthetic spec for testing.",
	})
	if err != nil {
		t.Fatalf("insert source file: %v", err)
	}

	// --- sections (hierarchical: §1, §1.1, §1.2, §2, §2.1) ---
	sec1ID, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "§1",
		Title:        "Core Requirements",
		HeadingLevel: 1,
		LineStart:    1,
		LineEnd:      100,
		RawText:      "# Core Requirements\n\nFundamental invariants governing system behavior.",
		ContentHash:  "sec1-hash",
	})
	if err != nil {
		t.Fatalf("insert section §1: %v", err)
	}

	sec11ID, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "§1.1",
		Title:        "Data Integrity",
		HeadingLevel: 2,
		ParentID:     &sec1ID,
		LineStart:    10,
		LineEnd:      50,
		RawText:      "## Data Integrity\n\nRules ensuring data persistence and correctness.",
		ContentHash:  "sec11-hash",
	})
	if err != nil {
		t.Fatalf("insert section §1.1: %v", err)
	}

	sec12ID, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "§1.2",
		Title:        "Validation Pipeline",
		HeadingLevel: 2,
		ParentID:     &sec1ID,
		LineStart:    51,
		LineEnd:      100,
		RawText:      "## Validation Pipeline\n\nMechanical checks for spec conformance.",
		ContentHash:  "sec12-hash",
	})
	if err != nil {
		t.Fatalf("insert section §1.2: %v", err)
	}

	sec2ID, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "§2",
		Title:        "Search Intelligence",
		HeadingLevel: 1,
		LineStart:    101,
		LineEnd:      200,
		RawText:      "# Search Intelligence\n\nMulti-signal retrieval and ranking.",
		ContentHash:  "sec2-hash",
	})
	if err != nil {
		t.Fatalf("insert section §2: %v", err)
	}

	sec21ID, err := storage.InsertSection(db, &storage.Section{
		SpecID:       specID,
		SourceFileID: sfID,
		SectionPath:  "§2.1",
		Title:        "BM25 Retrieval",
		HeadingLevel: 2,
		ParentID:     &sec2ID,
		LineStart:    110,
		LineEnd:      150,
		RawText:      "## BM25 Retrieval\n\nLexical search via FTS5 with BM25 scoring.",
		ContentHash:  "sec21-hash",
	})
	if err != nil {
		t.Fatalf("insert section §2.1: %v", err)
	}

	// --- invariants (5, all fields populated) ---
	invs := []storage.Invariant{
		{
			SpecID: specID, SourceFileID: sfID, SectionID: sec11ID,
			InvariantID: "INV-001", Title: "Round-Trip Fidelity",
			Statement:         "parse(render(parse(doc))) is byte-identical to parse(doc)",
			SemiFormal:        "∀ doc ∈ Documents: parse(render(parse(doc))) = parse(doc)",
			ViolationScenario: "A document parsed and rendered produces different bytes when re-parsed.",
			ValidationMethod:  "Parse a document, render it, re-parse and compare SHA-256 hashes.",
			WhyThisMatters:    "Ensures no information loss during round-trip.",
			LineStart: 15, LineEnd: 25, ContentHash: "inv001-hash",
			RawText: "**INV-NNN: Round-Trip Fidelity**",
		},
		{
			SpecID: specID, SourceFileID: sfID, SectionID: sec11ID,
			InvariantID: "INV-002", Title: "Deterministic Output",
			Statement:         "Given the same input, the parser produces byte-identical output every time.",
			SemiFormal:        "forall d. parse(d) = parse(d)",
			ViolationScenario: "Running the parser twice on identical input produces different database content.",
			ValidationMethod:  "Parse the same document twice, compare content hashes of each element.",
			WhyThisMatters:    "Non-determinism breaks reproducibility and CI trust.",
			LineStart: 26, LineEnd: 36, ContentHash: "inv002-hash",
			RawText: "**INV-NNN: Deterministic Output**",
		},
		{
			SpecID: specID, SourceFileID: sfID, SectionID: sec12ID,
			InvariantID: "INV-003", Title: "Progressive Validation",
			Statement:         "Validation checks are ordered: structural < semantic < inter-module.",
			SemiFormal:        "level(check_i) <= level(check_j) => i < j",
			ViolationScenario: "A semantic check runs before structural checks pass.",
			ValidationMethod:  "Verify check ordering in the validator implementation.",
			WhyThisMatters:    "Early exit on structural failures saves expensive semantic analysis.",
			LineStart: 55, LineEnd: 65, ContentHash: "inv003-hash",
			RawText: "**INV-NNN: Progressive Validation**",
		},
		{
			SpecID: specID, SourceFileID: sfID, SectionID: sec2ID,
			InvariantID: "INV-004", Title: "Search Recall",
			Statement:         "If a spec element matches a query by ID, it must appear in top-3 results.",
			SemiFormal:        "forall q. match_id(q, e) => rank(e, search(q)) <= 3",
			ViolationScenario: "Searching for INV-006 does not return INV-006 in the top 3.",
			ValidationMethod:  "Search for each invariant by ID, verify it appears in results.",
			WhyThisMatters:    "Users rely on ID-based search for navigation.",
			LineStart: 105, LineEnd: 115, ContentHash: "inv004-hash",
			RawText: "**INV-NNN: Search Recall**",
		},
		{
			SpecID: specID, SourceFileID: sfID, SectionID: sec21ID,
			InvariantID: "INV-005", Title: "Drift Detection Completeness",
			Statement:         "Every implementation element not in spec is flagged as unspecified.",
			SemiFormal:        "|impl \\ spec| = |unspecified|",
			ViolationScenario: "A function exists in code but drift analysis does not report it.",
			ValidationMethod:  "Compare scan annotations against spec elements, verify no omissions.",
			WhyThisMatters:    "Undetected drift accumulates technical debt.",
			LineStart: 115, LineEnd: 125, ContentHash: "inv005-hash",
			RawText: "**INV-NNN: Drift Detection Completeness**",
		},
	}
	for i := range invs {
		if _, err := storage.InsertInvariant(db, &invs[i]); err != nil {
			t.Fatalf("insert invariant %s: %v", invs[i].InvariantID, err)
		}
	}

	// --- ADRs (3) ---
	adrs := []storage.ADR{
		{
			SpecID: specID, SourceFileID: sfID, SectionID: sec11ID,
			ADRID: "ADR-001", Title: "SQLite over PostgreSQL",
			Problem:      "Which database to use for the spec index?",
			DecisionText: "Use SQLite for zero-dependency distribution.",
			ChosenOption: "SQLite",
			Consequences: "Single-file database, no server needed, but limited concurrent writes.",
			Tests:        "Verify all queries work on WAL mode.",
			Status:       "active",
			LineStart: 30, LineEnd: 40, ContentHash: "adr001-hash",
			RawText: "### ADR-001: SQLite over PostgreSQL",
		},
		{
			SpecID: specID, SourceFileID: sfID, SectionID: sec12ID,
			ADRID: "ADR-002", Title: "FTS5 for Full-Text Search",
			Problem:      "How to implement spec search?",
			DecisionText: "Use SQLite FTS5 extension for BM25 ranking.",
			ChosenOption: "FTS5",
			Consequences: "Fast lexical search, but no semantic understanding without LSI layer.",
			Tests:        "Verify BM25 scores increase for more relevant queries.",
			Status:       "active",
			LineStart: 70, LineEnd: 80, ContentHash: "adr002-hash",
			RawText: "### ADR-002: FTS5 for Full-Text Search",
		},
		{
			SpecID: specID, SourceFileID: sfID, SectionID: sec2ID,
			ADRID: "ADR-003", Title: "RRF over Linear Combination",
			Problem:      "How to fuse multiple search signals?",
			DecisionText: "Use Reciprocal Rank Fusion (RRF) instead of weighted linear combination.",
			ChosenOption: "RRF",
			Consequences: "Robust to score scale differences, but fixed k=60 parameter.",
			Tests:        "Verify RRF output is independent of score magnitude.",
			Status:       "active",
			LineStart: 130, LineEnd: 140, ContentHash: "adr003-hash",
			RawText: "### ADR-003: RRF over Linear Combination",
		},
	}
	for i := range adrs {
		if _, err := storage.InsertADR(db, &adrs[i]); err != nil {
			t.Fatalf("insert ADR %s: %v", adrs[i].ADRID, err)
		}
	}

	// --- quality_gates (2 for Check 10) ---
	gates := []storage.QualityGate{
		{
			SpecID: specID, SectionID: sec12ID,
			GateID: "Gate-1", Title: "Structural Conformance",
			Predicate: "All sections have unique paths AND all invariants have 6 components.",
			LineStart: 85, LineEnd: 90,
			RawText: "**Gate 1: Structural Conformance**",
		},
		{
			SpecID: specID, SectionID: sec12ID,
			GateID: "Gate-2", Title: "Cross-Reference Integrity",
			Predicate: "All cross-references resolve to existing elements.",
			LineStart: 91, LineEnd: 96,
			RawText: "**Gate 2: Cross-Reference Integrity**",
		},
	}
	for i := range gates {
		if _, err := storage.InsertQualityGate(db, &gates[i]); err != nil {
			t.Fatalf("insert gate %s: %v", gates[i].GateID, err)
		}
	}

	// --- negative_specs (5 for Check 10) ---
	negspecs := []storage.NegativeSpec{
		{SpecID: specID, SourceFileID: sfID, SectionID: sec11ID, ConstraintText: "DO NOT store credentials in the database.", LineNumber: 45, RawText: "**DO NOT** store credentials in the database."},
		{SpecID: specID, SourceFileID: sfID, SectionID: sec11ID, ConstraintText: "DO NOT allow SQL injection through user queries.", LineNumber: 46, RawText: "**DO NOT** allow SQL injection through user queries."},
		{SpecID: specID, SourceFileID: sfID, SectionID: sec12ID, ConstraintText: "DO NOT skip structural checks before semantic analysis.", LineNumber: 85, RawText: "**DO NOT** skip structural checks before semantic analysis."},
		{SpecID: specID, SourceFileID: sfID, SectionID: sec2ID, ConstraintText: "DO NOT return more than max_results without explicit pagination.", LineNumber: 120, RawText: "**DO NOT** return more than max_results without explicit pagination."},
		{SpecID: specID, SourceFileID: sfID, SectionID: sec21ID, ConstraintText: "DO NOT use stopwords in the FTS5 index.", LineNumber: 135, RawText: "**DO NOT** use stopwords in the FTS5 index."},
	}
	for i := range negspecs {
		if _, err := storage.InsertNegativeSpec(db, &negspecs[i]); err != nil {
			t.Fatalf("insert negative spec %d: %v", i, err)
		}
	}

	// --- cross_references (10: 8 resolved, 2 unresolved) ---
	xrefs := []storage.CrossReference{
		{SpecID: specID, SourceFileID: sfID, SourceSectionID: &sec11ID, SourceLine: 20, RefType: "invariant", RefTarget: "INV-001", RefText: "see INV-001", Resolved: true},
		{SpecID: specID, SourceFileID: sfID, SourceSectionID: &sec11ID, SourceLine: 22, RefType: "invariant", RefTarget: "INV-002", RefText: "see INV-002", Resolved: true},
		{SpecID: specID, SourceFileID: sfID, SourceSectionID: &sec12ID, SourceLine: 60, RefType: "invariant", RefTarget: "INV-003", RefText: "see INV-003", Resolved: true},
		{SpecID: specID, SourceFileID: sfID, SourceSectionID: &sec2ID, SourceLine: 110, RefType: "invariant", RefTarget: "INV-004", RefText: "see INV-004", Resolved: true},
		{SpecID: specID, SourceFileID: sfID, SourceSectionID: &sec21ID, SourceLine: 120, RefType: "invariant", RefTarget: "INV-005", RefText: "see INV-005", Resolved: true},
		{SpecID: specID, SourceFileID: sfID, SourceSectionID: &sec11ID, SourceLine: 35, RefType: "adr", RefTarget: "ADR-001", RefText: "see ADR-001", Resolved: true},
		{SpecID: specID, SourceFileID: sfID, SourceSectionID: &sec12ID, SourceLine: 75, RefType: "adr", RefTarget: "ADR-002", RefText: "see ADR-002", Resolved: true},
		{SpecID: specID, SourceFileID: sfID, SourceSectionID: &sec2ID, SourceLine: 135, RefType: "adr", RefTarget: "ADR-003", RefText: "see ADR-003", Resolved: true},
		{SpecID: specID, SourceFileID: sfID, SourceSectionID: &sec11ID, SourceLine: 42, RefType: "section", RefTarget: "§3.1", RefText: "see §3.1", Resolved: false},
		{SpecID: specID, SourceFileID: sfID, SourceSectionID: &sec2ID, SourceLine: 145, RefType: "invariant", RefTarget: "INV-099", RefText: "see INV-099", Resolved: false},
	}
	for i := range xrefs {
		if _, err := storage.InsertCrossReference(db, &xrefs[i]); err != nil {
			t.Fatalf("insert xref %d: %v", i, err)
		}
	}

	// --- glossary_entries (3) ---
	glossary := []storage.GlossaryEntry{
		{SpecID: specID, SectionID: sec11ID, Term: "Round-Trip Fidelity", Definition: "The property that parse(render(parse(x))) = parse(x).", LineNumber: 48},
		{SpecID: specID, SectionID: sec2ID, Term: "BM25", Definition: "Best Matching 25 — a probabilistic relevance scoring function.", LineNumber: 150},
		{SpecID: specID, SectionID: sec2ID, Term: "RRF", Definition: "Reciprocal Rank Fusion — combines ranked lists via 1/(k+rank).", LineNumber: 155},
	}
	for i := range glossary {
		if _, err := storage.InsertGlossaryEntry(db, &glossary[i]); err != nil {
			t.Fatalf("insert glossary %d: %v", i, err)
		}
	}

	// --- authority scores (for search ranking) ---
	authorities := map[string]float64{
		"INV-001": 0.15, "INV-002": 0.12, "INV-003": 0.10,
		"INV-004": 0.08, "INV-005": 0.05,
		"ADR-001": 0.13, "ADR-002": 0.09, "ADR-003": 0.07,
		"§1": 0.20, "§2": 0.18,
	}
	for elemID, score := range authorities {
		if err := storage.InsertAuthority(db, specID, elemID, score); err != nil {
			t.Fatalf("insert authority %s: %v", elemID, err)
		}
	}

	return db, specID
}

// buildSyntheticModularDB creates a modular spec DB with modules and relationships.
// Suitable for bundle, cascade, implorder, and progress tests.
func buildSyntheticModularDB(t *testing.T) (*sql.DB, int64) {
	t.Helper()

	db, specID := buildSyntheticDB(t)

	// Get the source file ID for the monolith entry
	var sfID int64
	err := db.QueryRow(`SELECT id FROM source_files WHERE spec_id = ? LIMIT 1`, specID).Scan(&sfID)
	if err != nil {
		t.Fatalf("get source file: %v", err)
	}

	// Add a constitution source file (required by bundle.Assemble)
	_, err = storage.InsertSourceFile(db, &storage.SourceFile{
		SpecID:      specID,
		FilePath:    "/test/system-constitution.md",
		FileRole:    "system_constitution",
		ContentHash: "const-hash",
		LineCount:   30,
		RawText: `# System Constitution

## Core Principles
Specifications are bilateral discourse between code and theory.
Self-bootstrapping at every level.

## State Space
The system tracks specification index, annotations, witnesses, events.

## Quality Gates
All specs must satisfy structural conformance and cross-reference integrity.
`,
	})
	if err != nil {
		t.Fatalf("insert constitution source file: %v", err)
	}

	// Add a second source file for the module
	sf2ID, err := storage.InsertSourceFile(db, &storage.SourceFile{
		SpecID:      specID,
		FilePath:    "/test/modules/search-intelligence.md",
		FileRole:    "module",
		ModuleName:  "search-intelligence",
		ContentHash: "mod-search-hash",
		LineCount:   200,
		RawText:     "# Search Intelligence Module\n\nMulti-signal search.",
	})
	if err != nil {
		t.Fatalf("insert module source file: %v", err)
	}

	// --- modules (2) ---
	mod1ID, err := storage.InsertModule(db, &storage.Module{
		SpecID:       specID,
		SourceFileID: sfID,
		ModuleName:   "parse-pipeline",
		Domain:       "parsing",
		LineCount:    300,
	})
	if err != nil {
		t.Fatalf("insert module parse-pipeline: %v", err)
	}

	mod2ID, err := storage.InsertModule(db, &storage.Module{
		SpecID:       specID,
		SourceFileID: sf2ID,
		ModuleName:   "search-intelligence",
		Domain:       "search",
		LineCount:    200,
	})
	if err != nil {
		t.Fatalf("insert module search-intelligence: %v", err)
	}

	// --- module_relationships ---
	rels := []storage.ModuleRelationship{
		{ModuleID: mod1ID, RelType: "maintains", Target: "INV-001"},
		{ModuleID: mod1ID, RelType: "maintains", Target: "INV-002"},
		{ModuleID: mod1ID, RelType: "maintains", Target: "INV-003"},
		{ModuleID: mod2ID, RelType: "maintains", Target: "INV-004"},
		{ModuleID: mod2ID, RelType: "maintains", Target: "INV-005"},
		{ModuleID: mod2ID, RelType: "interfaces", Target: "parse-pipeline"},
		{ModuleID: mod1ID, RelType: "adjacent", Target: "search-intelligence"},
	}
	for i := range rels {
		if _, err := storage.InsertModuleRelationship(db, &rels[i]); err != nil {
			t.Fatalf("insert module relationship %d: %v", i, err)
		}
	}

	// --- invariant_registry (maps invariants to owning module domains) ---
	regEntries := []storage.InvariantRegistryEntry{
		{SpecID: specID, InvariantID: "INV-001", Owner: "parse-pipeline", Domain: "parsing", Description: "Round-Trip Fidelity"},
		{SpecID: specID, InvariantID: "INV-002", Owner: "parse-pipeline", Domain: "parsing", Description: "Deterministic Output"},
		{SpecID: specID, InvariantID: "INV-003", Owner: "parse-pipeline", Domain: "parsing", Description: "Progressive Validation"},
		{SpecID: specID, InvariantID: "INV-004", Owner: "search-intelligence", Domain: "search", Description: "Search Recall"},
		{SpecID: specID, InvariantID: "INV-005", Owner: "search-intelligence", Domain: "search", Description: "Drift Detection Completeness"},
	}
	for i := range regEntries {
		if _, err := storage.InsertInvariantRegistryEntry(db, &regEntries[i]); err != nil {
			t.Fatalf("insert invariant registry %s: %v", regEntries[i].InvariantID, err)
		}
	}

	// Update source type to modular
	if _, err := db.Exec(`UPDATE spec_index SET source_type = 'modular' WHERE id = ?`, specID); err != nil {
		t.Fatalf("update source type: %v", err)
	}

	return db, specID
}

// buildSyntheticSearchDB creates a DB with FTS and LSI indices built.
// Suitable for search and exemplar tests.
func buildSyntheticSearchDB(t *testing.T) (*sql.DB, int64, *search.LSIIndex) {
	t.Helper()

	db, specID := buildSyntheticDB(t)

	// Build FTS5 + LSI + PageRank
	if err := search.BuildIndex(db, specID); err != nil {
		t.Fatalf("build search index: %v", err)
	}

	// Build LSI index from extracted docs (same pattern as existing tests)
	docs, err := search.ExtractDocuments(db, specID)
	if err != nil {
		t.Fatalf("extract docs: %v", err)
	}
	k := 50
	if len(docs) < k {
		k = len(docs)
	}
	lsi, err := search.BuildLSI(docs, k)
	if err != nil {
		t.Fatalf("build lsi: %v", err)
	}

	return db, specID, lsi
}
