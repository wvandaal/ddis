package oplog

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/wvandaal/ddis/internal/validator"
)

// DefaultPath returns the default oplog path for a given spec directory.
func DefaultPath(specDir string) string {
	return filepath.Join(specDir, ".ddis", "oplog.jsonl")
}

// Append writes one or more records as JSONL lines to the given file.
// Creates the parent directory if needed.
func Append(path string, records ...*Record) error {
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return fmt.Errorf("create oplog dir: %w", err)
	}

	f, err := os.OpenFile(path, os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)
	if err != nil {
		return fmt.Errorf("open oplog: %w", err)
	}
	defer f.Close()

	enc := json.NewEncoder(f)
	enc.SetEscapeHTML(false)
	for _, rec := range records {
		if err := enc.Encode(rec); err != nil {
			return fmt.Errorf("encode record: %w", err)
		}
	}
	return nil
}

// ReadAll reads every record from the oplog file.
// Returns an empty slice (not error) if the file doesn't exist.
func ReadAll(path string) ([]*Record, error) {
	return ReadFiltered(path, FilterOpts{})
}

// FilterOpts controls which records are returned by ReadFiltered.
type FilterOpts struct {
	Types []RecordType // empty = all types
	TxID  string       // empty = all transactions
	Since string       // RFC3339; empty = no lower bound
	Limit int          // 0 = unlimited
}

// ReadFiltered reads records from the oplog, applying optional filters.
func ReadFiltered(path string, opts FilterOpts) ([]*Record, error) {
	f, err := os.Open(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, nil
		}
		return nil, fmt.Errorf("open oplog: %w", err)
	}
	defer f.Close()

	wantTypes := make(map[RecordType]bool)
	for _, t := range opts.Types {
		wantTypes[t] = true
	}

	var sinceTime time.Time
	if opts.Since != "" {
		sinceTime, err = time.Parse(time.RFC3339, opts.Since)
		if err != nil {
			return nil, fmt.Errorf("parse --since time: %w", err)
		}
	}

	var records []*Record
	scanner := bufio.NewScanner(f)
	scanner.Buffer(make([]byte, 0, 1024*1024), 10*1024*1024) // 10MB max line

	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if line == "" {
			continue
		}

		var rec Record
		if err := json.Unmarshal([]byte(line), &rec); err != nil {
			return nil, fmt.Errorf("decode oplog line: %w", err)
		}

		// Apply filters
		if len(wantTypes) > 0 && !wantTypes[rec.Type] {
			continue
		}
		if opts.TxID != "" && rec.TxID != opts.TxID {
			continue
		}
		if !sinceTime.IsZero() {
			recTime, err := time.Parse(time.RFC3339, rec.Timestamp)
			if err == nil && recTime.Before(sinceTime) {
				continue
			}
		}

		records = append(records, &rec)
		if opts.Limit > 0 && len(records) >= opts.Limit {
			break
		}
	}

	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("scan oplog: %w", err)
	}
	return records, nil
}

// ImportValidation converts a validator.Report into a ValidateData record.
func ImportValidation(report *validator.Report, specPath, contentHash string) *ValidateData {
	vd := &ValidateData{
		SpecPath:    specPath,
		ContentHash: contentHash,
		TotalChecks: report.TotalChecks,
		Passed:      report.Passed,
		Failed:      report.Failed,
		Errors:      report.Errors,
		Warnings:    report.Warnings,
	}

	for _, r := range report.Results {
		vd.Results = append(vd.Results, ValidateResult{
			CheckID:   r.CheckID,
			CheckName: r.CheckName,
			Passed:    r.Passed,
			Summary:   r.Summary,
		})
	}
	return vd
}

// HasGenesisTransaction checks if the oplog already contains a genesis transaction.
func HasGenesisTransaction(path string) (bool, error) {
	records, err := ReadFiltered(path, FilterOpts{
		Types: []RecordType{RecordTypeTransaction},
	})
	if err != nil {
		return false, err
	}

	for _, rec := range records {
		td, err := rec.DecodeTx()
		if err != nil {
			continue
		}
		if td.Action == TxActionBegin && strings.HasPrefix(td.Description, "Genesis:") {
			return true, nil
		}
	}
	return false, nil
}

// RenderLog formats oplog records for human or JSON output.
func RenderLog(records []*Record, asJSON bool) (string, error) {
	if asJSON {
		data, err := json.MarshalIndent(records, "", "  ")
		if err != nil {
			return "", fmt.Errorf("marshal records: %w", err)
		}
		return string(data), nil
	}

	var b strings.Builder
	fmt.Fprintf(&b, "Operation Log (%d records)\n", len(records))
	b.WriteString("═══════════════════════════════════════════\n\n")

	for i, rec := range records {
		fmt.Fprintf(&b, "[%d] %s  type=%s", i+1, rec.Timestamp, rec.Type)
		if rec.TxID != "" {
			fmt.Fprintf(&b, "  tx=%s", rec.TxID)
		}
		b.WriteString("\n")

		switch rec.Type {
		case RecordTypeDiff:
			d, err := rec.DecodeDiff()
			if err == nil {
				fmt.Fprintf(&b, "    %s → %s\n", d.Base.SpecPath, d.Head.SpecPath)
				fmt.Fprintf(&b, "    +%d -%d ~%d =%d\n",
					d.Summary.Added, d.Summary.Removed, d.Summary.Modified, d.Summary.Unchanged)
			}

		case RecordTypeValidate:
			v, err := rec.DecodeValidate()
			if err == nil {
				fmt.Fprintf(&b, "    %s: %d checks, %d passed, %d failed (%d errors)\n",
					v.SpecPath, v.TotalChecks, v.Passed, v.Failed, v.Errors)
			}

		case RecordTypeTransaction:
			t, err := rec.DecodeTx()
			if err == nil {
				fmt.Fprintf(&b, "    action=%s", t.Action)
				if t.Description != "" {
					fmt.Fprintf(&b, "  %q", t.Description)
				}
				b.WriteString("\n")
			}
		}
		b.WriteString("\n")
	}

	return b.String(), nil
}
