//go:build integration

package tests

// Integration test helpers: shared DB caches, projectRoot(), and getter functions.
// These are only available when running with -tags integration.
// All functions here parse real spec files for self-bootstrapping verification.

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/wvandaal/ddis/internal/parser"
	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

// projectRoot returns the DDIS project root (parent of ddis-cli/).
func projectRoot() string {
	if root := os.Getenv("DDIS_PROJECT_ROOT"); root != "" {
		return root
	}
	return "/data/projects/ddis"
}

// --- Shared DB caches (lazy-initialized, one per test run) ---

type modularTestDB struct {
	db     storage.DB
	specID int64
}

var sharedModularDB *modularTestDB

func getModularDB(t *testing.T) (storage.DB, int64) {
	t.Helper()
	if sharedModularDB != nil {
		return sharedModularDB.db, sharedModularDB.specID
	}

	manifestPath := filepath.Join(projectRoot(), "ddis-cli-spec", "manifest.yaml")
	if _, err := os.Stat(manifestPath); os.IsNotExist(err) {
		t.Fatalf("ddis-cli-spec manifest not found: %s", manifestPath)
	}

	dbPath := filepath.Join(t.TempDir(), "modular_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := parser.ParseModularSpec(manifestPath, db)
	if err != nil {
		t.Fatalf("parse modular spec: %v", err)
	}

	sharedModularDB = &modularTestDB{db: db, specID: specID}
	return sharedModularDB.db, sharedModularDB.specID
}

type searchTestDB struct {
	db     *storage.DB
	specID int64
	lsi    *search.LSIIndex
}

var sharedSearchDB *searchTestDB

func getSearchDB(t *testing.T) (*storage.DB, int64, *search.LSIIndex) {
	t.Helper()
	if sharedSearchDB != nil {
		return sharedSearchDB.db, sharedSearchDB.specID, sharedSearchDB.lsi
	}

	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Fatalf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "search_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	if err := search.BuildIndex(db, specID); err != nil {
		t.Fatalf("build index: %v", err)
	}

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

	sharedSearchDB = &searchTestDB{db: &db, specID: specID, lsi: lsi}
	return sharedSearchDB.db, sharedSearchDB.specID, sharedSearchDB.lsi
}

type coverageTestDB struct {
	db     *storage.DB
	specID int64
}

var sharedCoverageDB *coverageTestDB

func getCoverageDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedCoverageDB != nil {
		return sharedCoverageDB.db, sharedCoverageDB.specID
	}

	manifestPath := filepath.Join(projectRoot(), "ddis-cli-spec", "manifest.yaml")
	monolithPath := filepath.Join(projectRoot(), "ddis_final.md")

	var specPath string
	var isModular bool
	if _, err := os.Stat(manifestPath); err == nil {
		specPath = manifestPath
		isModular = true
	} else if _, err := os.Stat(monolithPath); err == nil {
		specPath = monolithPath
	} else {
		t.Fatalf("no spec found (tried %s and %s)", manifestPath, monolithPath)
	}

	dbPath := filepath.Join(t.TempDir(), "coverage_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	var specID int64
	if isModular {
		specID, err = parser.ParseModularSpec(specPath, db)
	} else {
		specID, err = parser.ParseDocument(specPath, db)
	}
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	sharedCoverageDB = &coverageTestDB{db: &db, specID: specID}
	return sharedCoverageDB.db, sharedCoverageDB.specID
}

type driftTestDB struct {
	db     *storage.DB
	specID int64
}

var sharedDriftDB *driftTestDB

func getDriftDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedDriftDB != nil {
		return sharedDriftDB.db, sharedDriftDB.specID
	}

	manifestPath := filepath.Join(projectRoot(), "ddis-cli-spec", "manifest.yaml")
	monolithPath := filepath.Join(projectRoot(), "ddis_final.md")

	var specPath string
	var isModular bool
	if _, err := os.Stat(manifestPath); err == nil {
		specPath = manifestPath
		isModular = true
	} else if _, err := os.Stat(monolithPath); err == nil {
		specPath = monolithPath
	} else {
		t.Fatalf("no spec found (tried %s and %s)", manifestPath, monolithPath)
	}

	dbPath := filepath.Join(t.TempDir(), "drift_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	var specID int64
	if isModular {
		specID, err = parser.ParseModularSpec(specPath, db)
	} else {
		specID, err = parser.ParseDocument(specPath, db)
	}
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	sharedDriftDB = &driftTestDB{db: &db, specID: specID}
	return sharedDriftDB.db, sharedDriftDB.specID
}

type validateTestDB struct {
	db     *storage.DB
	specID int64
}

var sharedValidateDB *validateTestDB

func getValidateDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedValidateDB != nil {
		return sharedValidateDB.db, sharedValidateDB.specID
	}

	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Fatalf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "validate_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	sharedValidateDB = &validateTestDB{db: &db, specID: specID}
	return sharedValidateDB.db, sharedValidateDB.specID
}

type exemplarTestDB struct {
	db     *storage.DB
	specID int64
	lsi    *search.LSIIndex
}

var sharedExemplarDB *exemplarTestDB

func getExemplarDB(t *testing.T) (*storage.DB, int64, *search.LSIIndex) {
	t.Helper()
	if sharedExemplarDB != nil {
		return sharedExemplarDB.db, sharedExemplarDB.specID, sharedExemplarDB.lsi
	}

	manifestPath := filepath.Join(projectRoot(), "ddis-cli-spec", "manifest.yaml")
	monolithPath := filepath.Join(projectRoot(), "ddis_final.md")

	var specPath string
	var isModular bool
	if _, err := os.Stat(manifestPath); err == nil {
		specPath = manifestPath
		isModular = true
	} else if _, err := os.Stat(monolithPath); err == nil {
		specPath = monolithPath
	} else {
		t.Fatalf("no spec found (tried %s and %s)", manifestPath, monolithPath)
	}

	dbPath := filepath.Join(t.TempDir(), "exemplar_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	var specID int64
	if isModular {
		specID, err = parser.ParseModularSpec(specPath, db)
	} else {
		specID, err = parser.ParseDocument(specPath, db)
	}
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	if err := search.BuildIndex(db, specID); err != nil {
		t.Fatalf("build index: %v", err)
	}

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

	sharedExemplarDB = &exemplarTestDB{db: &db, specID: specID, lsi: lsi}
	return sharedExemplarDB.db, sharedExemplarDB.specID, sharedExemplarDB.lsi
}

type bundleTestDB struct {
	db     *storage.DB
	specID int64
}

var sharedBundleDB *bundleTestDB

func getBundleDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedBundleDB != nil {
		return sharedBundleDB.db, sharedBundleDB.specID
	}

	manifestPath := filepath.Join(projectRoot(), "ddis-cli-spec", "manifest.yaml")
	monolithPath := filepath.Join(projectRoot(), "ddis_final.md")

	var specPath string
	var isModular bool
	if _, err := os.Stat(manifestPath); err == nil {
		specPath = manifestPath
		isModular = true
	} else if _, err := os.Stat(monolithPath); err == nil {
		specPath = monolithPath
	} else {
		t.Fatalf("no spec found (tried %s and %s)", manifestPath, monolithPath)
	}

	dbPath := filepath.Join(t.TempDir(), "bundle_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	var specID int64
	if isModular {
		specID, err = parser.ParseModularSpec(specPath, db)
	} else {
		specID, err = parser.ParseDocument(specPath, db)
	}
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	sharedBundleDB = &bundleTestDB{db: &db, specID: specID}
	return sharedBundleDB.db, sharedBundleDB.specID
}

type cascadeTestDB struct {
	db     *storage.DB
	specID int64
}

var sharedCascadeDB *cascadeTestDB

func getCascadeDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedCascadeDB != nil {
		return sharedCascadeDB.db, sharedCascadeDB.specID
	}

	manifestPath := filepath.Join(projectRoot(), "ddis-cli-spec", "manifest.yaml")
	if _, err := os.Stat(manifestPath); os.IsNotExist(err) {
		t.Fatalf("ddis-cli-spec manifest not found: %s", manifestPath)
	}

	dbPath := filepath.Join(t.TempDir(), "cascade_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := parser.ParseModularSpec(manifestPath, db)
	if err != nil {
		t.Fatalf("parse modular spec: %v", err)
	}

	sharedCascadeDB = &cascadeTestDB{db: &db, specID: specID}
	return sharedCascadeDB.db, sharedCascadeDB.specID
}

type implorderTestDB struct {
	db     *storage.DB
	specID int64
}

var sharedImplOrderDB *implorderTestDB

func getImplOrderDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedImplOrderDB != nil {
		return sharedImplOrderDB.db, sharedImplOrderDB.specID
	}

	manifestPath := filepath.Join(projectRoot(), "ddis-cli-spec", "manifest.yaml")
	monolithPath := filepath.Join(projectRoot(), "ddis_final.md")

	var specPath string
	var isModular bool
	if _, err := os.Stat(manifestPath); err == nil {
		specPath = manifestPath
		isModular = true
	} else if _, err := os.Stat(monolithPath); err == nil {
		specPath = monolithPath
	} else {
		t.Fatalf("no spec found (tried %s and %s)", manifestPath, monolithPath)
	}

	dbPath := filepath.Join(t.TempDir(), "implorder_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	var specID int64
	if isModular {
		specID, err = parser.ParseModularSpec(specPath, db)
	} else {
		specID, err = parser.ParseDocument(specPath, db)
	}
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	sharedImplOrderDB = &implorderTestDB{db: &db, specID: specID}
	return sharedImplOrderDB.db, sharedImplOrderDB.specID
}

type impactTestDB struct {
	db     *storage.DB
	specID int64
}

var sharedImpactDB *impactTestDB

func getImpactDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedImpactDB != nil {
		return sharedImpactDB.db, sharedImpactDB.specID
	}

	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Fatalf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "impact_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	sharedImpactDB = &impactTestDB{db: &db, specID: specID}
	return sharedImpactDB.db, sharedImpactDB.specID
}

type queryTestDB struct {
	db     *storage.DB
	specID int64
	dbPath string
}

var sharedQueryDB *queryTestDB

func getQueryDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedQueryDB != nil {
		return sharedQueryDB.db, sharedQueryDB.specID
	}

	specPath := filepath.Join(projectRoot(), "ddis_final.md")
	if _, err := os.Stat(specPath); os.IsNotExist(err) {
		t.Fatalf("ddis_final.md not found at %s", specPath)
	}

	dbPath := filepath.Join(t.TempDir(), "query_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := parser.ParseDocument(specPath, db)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	sharedQueryDB = &queryTestDB{db: &db, specID: specID, dbPath: dbPath}
	return sharedQueryDB.db, sharedQueryDB.specID
}

type xrefTestDB struct {
	db     *storage.DB
	specID int64
}

var sharedXRefDB *xrefTestDB

func getXRefDB(t *testing.T) (*storage.DB, int64) {
	t.Helper()
	if sharedXRefDB != nil {
		return sharedXRefDB.db, sharedXRefDB.specID
	}

	manifestPath := filepath.Join(projectRoot(), "ddis-cli-spec", "manifest.yaml")
	if _, err := os.Stat(manifestPath); os.IsNotExist(err) {
		t.Fatalf("ddis-cli-spec manifest not found: %s", manifestPath)
	}

	dbPath := filepath.Join(t.TempDir(), "xref_test.db")
	db, err := storage.Open(dbPath)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	specID, err := parser.ParseModularSpec(manifestPath, db)
	if err != nil {
		t.Fatalf("parse modular spec: %v", err)
	}

	sharedXRefDB = &xrefTestDB{db: &db, specID: specID}
	return sharedXRefDB.db, sharedXRefDB.specID
}
