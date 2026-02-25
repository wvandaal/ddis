package parser

// ddis:implements APP-ADR-010 (monolith/modular polymorphism)

import (
	"fmt"
	"os"
	"path/filepath"

	"gopkg.in/yaml.v3"

	"github.com/wvandaal/ddis/internal/storage"
)

// ManifestData represents the parsed manifest.yaml.
type ManifestData struct {
	DDISVersion string `yaml:"ddis_version"`
	SpecName    string `yaml:"spec_name"`
	TierMode    string `yaml:"tier_mode"`
	ParentSpec  string `yaml:"parent_spec"`

	ContextBudget struct {
		TargetLines      int     `yaml:"target_lines"`
		HardCeilingLines int     `yaml:"hard_ceiling_lines"`
		ReasoningReserve float64 `yaml:"reasoning_reserve"`
	} `yaml:"context_budget"`

	Constitution struct {
		System  string            `yaml:"system"`
		Domains map[string]string `yaml:"domains,omitempty"`
	} `yaml:"constitution"`

	Modules map[string]ModuleDecl `yaml:"modules"`

	InvariantRegistry map[string]InvRegistryEntry `yaml:"invariant_registry"`
}

// ModuleDecl represents one module entry in the manifest.
type ModuleDecl struct {
	File          string   `yaml:"file"`
	Domain        string   `yaml:"domain"`
	Maintains     []string `yaml:"maintains"`
	Interfaces    []string `yaml:"interfaces"`
	Implements    []string `yaml:"implements"`
	Adjacent      []string `yaml:"adjacent"`
	DeepContext   *string  `yaml:"deep_context"`
	NegativeSpecs []string `yaml:"negative_specs"`
}

// InvRegistryEntry represents one invariant in the registry.
type InvRegistryEntry struct {
	Owner       string `yaml:"owner"`
	Domain      string `yaml:"domain"`
	Description string `yaml:"description"`
}

// ParseManifestFile reads and parses a manifest.yaml file.
func ParseManifestFile(path string) (*ManifestData, string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, "", fmt.Errorf("read manifest: %w", err)
	}

	var m ManifestData
	if err := yaml.Unmarshal(data, &m); err != nil {
		return nil, "", fmt.Errorf("parse manifest YAML: %w", err)
	}

	return &m, string(data), nil
}

// ParseModularSpec parses a modular spec starting from manifest.yaml.
func ParseModularSpec(manifestPath string, db storage.DB) (int64, error) {
	manifestDir := filepath.Dir(manifestPath)

	manifest, rawYAML, err := ParseManifestFile(manifestPath)
	if err != nil {
		return 0, err
	}

	// Read all source files to compute total content hash
	allContent := rawYAML

	// Create spec index
	specIndex := &storage.SpecIndex{
		SpecPath:    manifestPath,
		SpecName:    manifest.SpecName,
		DDISVersion: manifest.DDISVersion,
		ContentHash: sha256Hex(allContent),
		ParsedAt:    nowISO8601(),
		SourceType:  "modular",
	}

	specID, err := storage.InsertSpecIndex(db, specIndex)
	if err != nil {
		return 0, err
	}

	// Insert manifest record
	mRec := &storage.Manifest{
		SpecID:           specID,
		DDISVersion:      manifest.DDISVersion,
		SpecName:         manifest.SpecName,
		TierMode:         manifest.TierMode,
		TargetLines:      manifest.ContextBudget.TargetLines,
		HardCeilingLines: manifest.ContextBudget.HardCeilingLines,
		ReasoningReserve: manifest.ContextBudget.ReasoningReserve,
		RawYAML:          rawYAML,
	}
	if _, err := storage.InsertManifest(db, mRec); err != nil {
		return 0, err
	}

	// Insert manifest as source file
	manifestSF := &storage.SourceFile{
		SpecID:      specID,
		FilePath:    "manifest.yaml",
		FileRole:    "manifest",
		ContentHash: sha256Hex(rawYAML),
		LineCount:   countLines(rawYAML),
		RawText:     rawYAML,
	}
	if _, err := storage.InsertSourceFile(db, manifestSF); err != nil {
		return 0, err
	}

	// Insert invariant registry entries
	for invID, entry := range manifest.InvariantRegistry {
		ire := &storage.InvariantRegistryEntry{
			SpecID:      specID,
			InvariantID: invID,
			Owner:       entry.Owner,
			Domain:      entry.Domain,
			Description: entry.Description,
		}
		if _, err := storage.InsertInvariantRegistryEntry(db, ire); err != nil {
			return 0, err
		}
	}

	totalLines := 0

	// Parse constitution
	if manifest.Constitution.System != "" {
		constitutionPath := filepath.Join(manifestDir, manifest.Constitution.System)
		sfID, lines, err := parseAndInsertFile(db, specID, constitutionPath,
			manifest.Constitution.System, "system_constitution", "")
		if err != nil {
			return 0, fmt.Errorf("parse constitution: %w", err)
		}
		totalLines += len(lines)
		if err := extractElementsFromFile(lines, specID, sfID, db); err != nil {
			return 0, fmt.Errorf("extract constitution elements: %w", err)
		}
	}

	// Parse each module
	for moduleName, moduleDecl := range manifest.Modules {
		modulePath := filepath.Join(manifestDir, moduleDecl.File)
		sfID, lines, err := parseAndInsertFile(db, specID, modulePath,
			moduleDecl.File, "module", moduleName)
		if err != nil {
			return 0, fmt.Errorf("parse module %s: %w", moduleName, err)
		}
		totalLines += len(lines)

		if err := extractElementsFromFile(lines, specID, sfID, db); err != nil {
			return 0, fmt.Errorf("extract module %s elements: %w", moduleName, err)
		}

		// Insert module record
		mod := &storage.Module{
			SpecID:       specID,
			SourceFileID: sfID,
			ModuleName:   moduleName,
			Domain:       moduleDecl.Domain,
			LineCount:    len(lines),
		}
		if moduleDecl.DeepContext != nil {
			mod.DeepContextPath = *moduleDecl.DeepContext
		}

		modID, err := storage.InsertModule(db, mod)
		if err != nil {
			return 0, fmt.Errorf("insert module %s: %w", moduleName, err)
		}

		// Insert relationships
		for _, inv := range moduleDecl.Maintains {
			mr := &storage.ModuleRelationship{ModuleID: modID, RelType: "maintains", Target: inv}
			if _, err := storage.InsertModuleRelationship(db, mr); err != nil {
				return 0, err
			}
		}
		for _, inv := range moduleDecl.Interfaces {
			mr := &storage.ModuleRelationship{ModuleID: modID, RelType: "interfaces", Target: inv}
			if _, err := storage.InsertModuleRelationship(db, mr); err != nil {
				return 0, err
			}
		}
		for _, adr := range moduleDecl.Implements {
			mr := &storage.ModuleRelationship{ModuleID: modID, RelType: "implements", Target: adr}
			if _, err := storage.InsertModuleRelationship(db, mr); err != nil {
				return 0, err
			}
		}
		for _, adj := range moduleDecl.Adjacent {
			mr := &storage.ModuleRelationship{ModuleID: modID, RelType: "adjacent", Target: adj}
			if _, err := storage.InsertModuleRelationship(db, mr); err != nil {
				return 0, err
			}
		}

		// Insert module negative specs
		for _, ns := range moduleDecl.NegativeSpecs {
			mns := &storage.ModuleNegativeSpec{ModuleID: modID, ConstraintText: ns}
			if _, err := storage.InsertModuleNegativeSpec(db, mns); err != nil {
				return 0, err
			}
		}
	}

	// Update total lines
	if _, err := db.Exec(`UPDATE spec_index SET total_lines = ? WHERE id = ?`, totalLines, specID); err != nil {
		return 0, err
	}

	// Parse parent spec if declared
	if manifest.ParentSpec != "" {
		parentPath := filepath.Join(filepath.Dir(manifestPath), manifest.ParentSpec)
		parentPath, err = filepath.Abs(parentPath)
		if err != nil {
			return 0, fmt.Errorf("resolve parent path: %w", err)
		}
		// Guard against infinite recursion
		absManifest, _ := filepath.Abs(manifestPath)
		if parentPath != absManifest {
			parentSpecID, err := ParseModularSpec(parentPath, db)
			if err != nil {
				return 0, fmt.Errorf("parse parent spec %s: %w", parentPath, err)
			}
			if err := storage.SetParentSpecID(db, specID, parentSpecID); err != nil {
				return 0, fmt.Errorf("set parent spec ID: %w", err)
			}
		}
	}

	// Resolve cross-references across all files (with parent fallback)
	if err := ResolveCrossReferences(db, specID); err != nil {
		return 0, fmt.Errorf("resolve cross-references: %w", err)
	}

	return specID, nil
}

// parseAndInsertFile reads a file, inserts it as a source file, builds sections.
func parseAndInsertFile(db storage.DB, specID int64, fullPath, relPath, role, moduleName string) (int64, []string, error) {
	content, err := os.ReadFile(fullPath)
	if err != nil {
		return 0, nil, fmt.Errorf("read %s: %w", fullPath, err)
	}

	text := string(content)
	lines := splitLines(text)

	sf := &storage.SourceFile{
		SpecID:      specID,
		FilePath:    relPath,
		FileRole:    role,
		ModuleName:  moduleName,
		ContentHash: sha256Hex(text),
		LineCount:   len(lines),
		RawText:     text,
	}

	sfID, err := storage.InsertSourceFile(db, sf)
	if err != nil {
		return 0, nil, err
	}

	// Build and insert sections
	sections := BuildSectionTree(lines)
	if err := InsertSectionsDB(db, sections, specID, sfID, lines); err != nil {
		return 0, nil, err
	}

	return sfID, lines, nil
}

func countLines(s string) int {
	if s == "" {
		return 0
	}
	n := 1
	for _, c := range s {
		if c == '\n' {
			n++
		}
	}
	return n
}
