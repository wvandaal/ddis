package workspace

// ddis:implements APP-ADR-026 (full workspace init)
// ddis:implements APP-ADR-028 (progressive validation over binary pass/fail)
// ddis:maintains APP-INV-037 (workspace isolation)
// ddis:maintains APP-INV-048 (event stream VCS primacy — init creates stream-{1,2,3}.jsonl)

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/wvandaal/ddis/internal/storage"
)

// InitOptions controls workspace initialization.
type InitOptions struct {
	Root          string // workspace root directory
	Workspace     bool   // create workspace.yaml for multi-spec management
	SkeletonLevel int    // template maturity level (1-3, default 1)
	SpecName      string // name for the spec (used in manifest)
}

// InitResult describes the initialized workspace.
type InitResult struct {
	Root    string   `json:"root"`    // absolute workspace root
	Created []string `json:"created"` // files created (relative paths)
	Skipped []string `json:"skipped"` // files that already existed (relative paths)
}

// Init initializes a DDIS workspace at the configured root directory.
// It is idempotent: existing files are never overwritten. Every path is
// verified to be confined within the resolved root before creation.
func Init(opts InitOptions) (*InitResult, error) {
	// Resolve root to absolute path.
	root, err := filepath.Abs(opts.Root)
	if err != nil {
		return nil, fmt.Errorf("resolve workspace root: %w", err)
	}

	// Ensure root itself exists.
	if err := os.MkdirAll(root, 0755); err != nil {
		return nil, fmt.Errorf("create workspace root: %w", err)
	}

	specName := opts.SpecName
	if specName == "" {
		specName = "untitled"
	}

	level := opts.SkeletonLevel
	if level < 1 || level > 3 {
		level = 1
	}

	result := &InitResult{Root: root}

	// --- files to create (in order) ---

	// 1. manifest.yaml
	if err := writeFileIfNew(root, "manifest.yaml", manifestContent(specName), result); err != nil {
		return nil, err
	}

	// 2. constitution/system.md
	if err := writeFileIfNew(root, "constitution/system.md", constitutionContent(level), result); err != nil {
		return nil, err
	}

	// 3. modules/ directory
	if err := mkdirIfNew(root, "modules", result); err != nil {
		return nil, err
	}

	// 4. .ddis/index.db — initialized via storage.Open
	dbRel := filepath.Join(".ddis", "index.db")
	dbAbs := filepath.Join(root, dbRel)
	if err := confine(dbAbs, root); err != nil {
		return nil, err
	}
	if _, err := os.Stat(dbAbs); os.IsNotExist(err) {
		if err := os.MkdirAll(filepath.Dir(dbAbs), 0755); err != nil {
			return nil, fmt.Errorf("create directory for %s: %w", dbRel, err)
		}
		db, err := storage.Open(dbAbs)
		if err != nil {
			return nil, fmt.Errorf("initialize index database: %w", err)
		}
		db.Close()
		result.Created = append(result.Created, dbRel)
	} else if err == nil {
		result.Skipped = append(result.Skipped, dbRel)
	} else {
		return nil, fmt.Errorf("stat %s: %w", dbRel, err)
	}

	// 5. .ddis/oplog.jsonl
	if err := writeFileIfNew(root, filepath.Join(".ddis", "oplog.jsonl"), "", result); err != nil {
		return nil, err
	}

	// 6. .ddis/events/*.jsonl
	for _, name := range []string{"stream-1.jsonl", "stream-2.jsonl", "stream-3.jsonl"} {
		rel := filepath.Join(".ddis", "events", name)
		if err := writeFileIfNew(root, rel, "", result); err != nil {
			return nil, err
		}
	}

	// 7. .ddis/discoveries/ directory
	if err := mkdirIfNew(root, filepath.Join(".ddis", "discoveries"), result); err != nil {
		return nil, err
	}

	// 8. .gitignore — append entries if missing
	if err := ensureGitignore(root, result); err != nil {
		return nil, err
	}

	// 9. Optional: .ddis/workspace.yaml
	if opts.Workspace {
		if err := writeFileIfNew(root, filepath.Join(".ddis", "workspace.yaml"), workspaceContent(), result); err != nil {
			return nil, err
		}
	}

	return result, nil
}

// ---------- path confinement ----------

func confine(absPath, root string) error {
	if !strings.HasPrefix(absPath, root) {
		return fmt.Errorf("path %q escapes workspace root %q", absPath, root)
	}
	return nil
}

// ---------- file/dir helpers ----------

// writeFileIfNew writes content to root/rel only if the file does not already exist.
func writeFileIfNew(root, rel, content string, result *InitResult) error {
	abs := filepath.Join(root, rel)
	if err := confine(abs, root); err != nil {
		return err
	}
	if _, err := os.Stat(abs); err == nil {
		result.Skipped = append(result.Skipped, rel)
		return nil
	} else if !os.IsNotExist(err) {
		return fmt.Errorf("stat %s: %w", rel, err)
	}
	if err := os.MkdirAll(filepath.Dir(abs), 0755); err != nil {
		return fmt.Errorf("create directory for %s: %w", rel, err)
	}
	if err := os.WriteFile(abs, []byte(content), 0644); err != nil {
		return fmt.Errorf("write %s: %w", rel, err)
	}
	result.Created = append(result.Created, rel)
	return nil
}

// mkdirIfNew creates a directory at root/rel only if it does not already exist.
func mkdirIfNew(root, rel string, result *InitResult) error {
	abs := filepath.Join(root, rel)
	if err := confine(abs, root); err != nil {
		return err
	}
	if info, err := os.Stat(abs); err == nil && info.IsDir() {
		result.Skipped = append(result.Skipped, rel)
		return nil
	}
	if err := os.MkdirAll(abs, 0755); err != nil {
		return fmt.Errorf("create directory %s: %w", rel, err)
	}
	result.Created = append(result.Created, rel)
	return nil
}

// ---------- .gitignore ----------

var gitignoreEntries = []string{
	".ddis/index.db",
}

const gitignoreBlock = `# DDIS derived artifacts
.ddis/index.db
`

func ensureGitignore(root string, result *InitResult) error {
	rel := ".gitignore"
	abs := filepath.Join(root, rel)
	if err := confine(abs, root); err != nil {
		return err
	}

	existing, err := os.ReadFile(abs)
	if err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("read .gitignore: %w", err)
	}

	if os.IsNotExist(err) {
		// Create new .gitignore with our entries.
		if err := os.WriteFile(abs, []byte(gitignoreBlock), 0644); err != nil {
			return fmt.Errorf("write .gitignore: %w", err)
		}
		result.Created = append(result.Created, rel)
		return nil
	}

	// .gitignore exists — append missing entries.
	content := string(existing)
	var toAdd []string
	for _, entry := range gitignoreEntries {
		if !strings.Contains(content, entry) {
			toAdd = append(toAdd, entry)
		}
	}
	if len(toAdd) == 0 {
		result.Skipped = append(result.Skipped, rel)
		return nil
	}

	// Ensure we start on a new line.
	if len(content) > 0 && !strings.HasSuffix(content, "\n") {
		content += "\n"
	}
	content += "\n# DDIS derived artifacts\n"
	for _, entry := range toAdd {
		content += entry + "\n"
	}
	if err := os.WriteFile(abs, []byte(content), 0644); err != nil {
		return fmt.Errorf("append .gitignore: %w", err)
	}
	result.Created = append(result.Created, rel)
	return nil
}

// ---------- template content ----------

func manifestContent(specName string) string {
	return fmt.Sprintf(`ddis_version: "3.0"
spec_name: %q
tier_mode: modular

constitution:
  system: "constitution/system.md"

modules: {}
`, specName)
}

func workspaceContent() string {
	return `# DDIS Workspace Configuration
loaded_specs: []
relationships: []
`
}

func constitutionContent(level int) string {
	if level >= 3 {
		return constitutionLevel3
	}
	return constitutionLevel1
}

const constitutionLevel1 = `---
module: system-constitution
domain: system
tier: 1
---

# System Constitution

## §0.1 Overview

[Describe the system's purpose, scope, and core guarantees. This section should
provide enough context for any reader to understand what the system does and why
its invariants matter.]

## §0.2 Invariants

### INV-001: [Title]

**Statement:** [What must always be true about the system.]

## §0.3 Architecture Decisions

### ADR-001: [Title]

**Problem:** [What decision needs to be made and why.]

**Decision:** [What was decided and the rationale.]
`

const constitutionLevel3 = `---
module: system-constitution
domain: system
tier: 3
---

# System Constitution

## §0.1 Overview

[Describe the system's purpose, scope, and core guarantees. This section should
provide enough context for any reader to understand what the system does and why
its invariants matter.]

## §0.2 Invariants

### INV-001: [Title]

**Statement:** [What must always be true about the system.]

**Semi-formal:** [Formal or semi-formal restatement (e.g., predicate logic, state machine constraint).]

**Violation scenario:** [A concrete example of what happens when this invariant is broken.]

**Validation method:** [How to mechanically verify this invariant holds.]

## §0.3 Architecture Decisions

### ADR-001: [Title]

**Problem:** [What decision needs to be made and why.]

**Options:** [Alternatives considered.]

**Decision:** [What was decided and the rationale.]

**Consequences:** [Trade-offs, risks, and follow-up work.]

**Tests:** [How to verify the decision is upheld.]
`
