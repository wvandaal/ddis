package diff

// ddis:implements APP-ADR-008 (surgical edit strategy)

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/wvandaal/ddis/internal/oplog"
	"github.com/wvandaal/ddis/internal/storage"
)

// DiffResult holds the complete structural diff between two spec indexes.
type DiffResult struct {
	Base    oplog.SpecRef     `json:"base"`
	Head    oplog.SpecRef     `json:"head"`
	Summary oplog.DiffSummary `json:"summary"`
	Changes []oplog.Change    `json:"changes"`
}

// ComputeDiff performs a structural comparison between two spec indexes.
func ComputeDiff(baseDB, headDB *sql.DB, baseSpec, headSpec int64) (*DiffResult, error) {
	baseIndex, err := storage.GetSpecIndex(baseDB, baseSpec)
	if err != nil {
		return nil, fmt.Errorf("get base spec: %w", err)
	}
	headIndex, err := storage.GetSpecIndex(headDB, headSpec)
	if err != nil {
		return nil, fmt.Errorf("get head spec: %w", err)
	}

	pairs, err := MatchElements(baseDB, headDB, baseSpec, headSpec)
	if err != nil {
		return nil, fmt.Errorf("match elements: %w", err)
	}

	result := &DiffResult{
		Base: oplog.SpecRef{
			SpecPath:    baseIndex.SpecPath,
			ContentHash: baseIndex.ContentHash,
		},
		Head: oplog.SpecRef{
			SpecPath:    headIndex.SpecPath,
			ContentHash: headIndex.ContentHash,
		},
	}

	for _, pair := range pairs {
		switch {
		case pair.BaseDBID == nil:
			// Added in head
			result.Summary.Added++
			result.Changes = append(result.Changes, oplog.Change{
				ElementType:      pair.ElementType,
				ElementID:        pair.ElementID,
				Action:           "added",
				ContentHashAfter: pair.HeadHash,
			})

		case pair.HeadDBID == nil:
			// Removed from head
			result.Summary.Removed++
			result.Changes = append(result.Changes, oplog.Change{
				ElementType:       pair.ElementType,
				ElementID:         pair.ElementID,
				Action:            "removed",
				ContentHashBefore: pair.BaseHash,
			})

		case pair.BaseHash != pair.HeadHash:
			// Modified
			result.Summary.Modified++
			result.Changes = append(result.Changes, oplog.Change{
				ElementType:       pair.ElementType,
				ElementID:         pair.ElementID,
				Action:            "modified",
				ContentHashBefore: pair.BaseHash,
				ContentHashAfter:  pair.HeadHash,
			})

		default:
			// Unchanged
			result.Summary.Unchanged++
		}
	}

	return result, nil
}

// ToDiffData converts a DiffResult to an oplog DiffData record payload.
func (d *DiffResult) ToDiffData() *oplog.DiffData {
	return &oplog.DiffData{
		Base:    d.Base,
		Head:    d.Head,
		Summary: d.Summary,
		Changes: d.Changes,
	}
}

// RenderDiff formats a DiffResult for human or JSON output.
func RenderDiff(result *DiffResult, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(result, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal diff: %w", err)
		}
		return string(data), nil
	}

	return renderHumanDiff(result), nil
}

func renderHumanDiff(d *DiffResult) string {
	var b strings.Builder

	fmt.Fprintf(&b, "Structural Diff: %s → %s\n", d.Base.SpecPath, d.Head.SpecPath)
	b.WriteString("═══════════════════════════════════════════\n\n")

	fmt.Fprintf(&b, "Summary: +%d added, -%d removed, ~%d modified, =%d unchanged\n\n",
		d.Summary.Added, d.Summary.Removed, d.Summary.Modified, d.Summary.Unchanged)

	if len(d.Changes) == 0 {
		b.WriteString("No structural changes detected.\n")
		return b.String()
	}

	// Group by action
	grouped := map[string][]oplog.Change{
		"added":    {},
		"removed":  {},
		"modified": {},
	}
	for _, c := range d.Changes {
		grouped[c.Action] = append(grouped[c.Action], c)
	}

	if added := grouped["added"]; len(added) > 0 {
		fmt.Fprintf(&b, "Added (%d):\n", len(added))
		for _, c := range added {
			fmt.Fprintf(&b, "  + [%s] %s\n", c.ElementType, c.ElementID)
		}
		b.WriteString("\n")
	}

	if removed := grouped["removed"]; len(removed) > 0 {
		fmt.Fprintf(&b, "Removed (%d):\n", len(removed))
		for _, c := range removed {
			fmt.Fprintf(&b, "  - [%s] %s\n", c.ElementType, c.ElementID)
		}
		b.WriteString("\n")
	}

	if modified := grouped["modified"]; len(modified) > 0 {
		fmt.Fprintf(&b, "Modified (%d):\n", len(modified))
		for _, c := range modified {
			detail := ""
			if c.Detail != "" {
				detail = " — " + c.Detail
			}
			fmt.Fprintf(&b, "  ~ [%s] %s%s\n", c.ElementType, c.ElementID, detail)
		}
		b.WriteString("\n")
	}

	return b.String()
}
