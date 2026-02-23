package oplog

import (
	"encoding/json"
	"fmt"
	"time"
)

// RecordType identifies the kind of oplog record.
type RecordType string

const (
	RecordTypeDiff        RecordType = "diff"
	RecordTypeValidate    RecordType = "validate"
	RecordTypeTransaction RecordType = "transaction"
)

// RecordVersion is the current schema version for oplog records.
const RecordVersion = 1

// Record is the envelope for every JSONL line. Type-specific data lives in Data.
type Record struct {
	Version   int             `json:"version"`
	Type      RecordType      `json:"type"`
	Timestamp string          `json:"timestamp"`
	TxID      string          `json:"tx_id,omitempty"`
	Data      json.RawMessage `json:"data"`
}

// SpecRef identifies a specific version of a spec file.
type SpecRef struct {
	SpecPath    string `json:"spec_path"`
	ContentHash string `json:"content_hash"`
}

// DiffSummary holds aggregate change counts.
type DiffSummary struct {
	Added     int `json:"added"`
	Removed   int `json:"removed"`
	Modified  int `json:"modified"`
	Unchanged int `json:"unchanged"`
}

// Change describes one element-level difference.
type Change struct {
	ElementType       string `json:"element_type"`
	ElementID         string `json:"element_id"`
	Action            string `json:"action"` // "added", "removed", "modified"
	SectionPath       string `json:"section_path,omitempty"`
	ContentHashBefore string `json:"content_hash_before,omitempty"`
	ContentHashAfter  string `json:"content_hash_after,omitempty"`
	Detail            string `json:"detail,omitempty"`
}

// DiffData is the type-specific payload for a "diff" record.
type DiffData struct {
	Base    SpecRef     `json:"base"`
	Head    SpecRef     `json:"head"`
	Summary DiffSummary `json:"summary"`
	Changes []Change    `json:"changes"`
}

// ValidateResult is one check result within a validate record.
type ValidateResult struct {
	CheckID   int    `json:"check_id"`
	CheckName string `json:"check_name"`
	Passed    bool   `json:"passed"`
	Summary   string `json:"summary"`
}

// ValidateData is the type-specific payload for a "validate" record.
type ValidateData struct {
	SpecPath    string           `json:"spec_path"`
	ContentHash string           `json:"content_hash"`
	TotalChecks int              `json:"total_checks"`
	Passed      int              `json:"passed"`
	Failed      int              `json:"failed"`
	Errors      int              `json:"errors"`
	Warnings    int              `json:"warnings"`
	Results     []ValidateResult `json:"results"`
}

// TxAction identifies a transaction lifecycle event.
type TxAction string

const (
	TxActionBegin    TxAction = "begin"
	TxActionCommit   TxAction = "commit"
	TxActionRollback TxAction = "rollback"
)

// TxData is the type-specific payload for a "transaction" record.
type TxData struct {
	Action      TxAction `json:"action"`
	Description string   `json:"description,omitempty"`
	ParentTxID  string   `json:"parent_tx_id,omitempty"`
}

// Now returns the current time in RFC3339 format.
func Now() string {
	return time.Now().UTC().Format(time.RFC3339)
}

// NewDiffRecord creates a diff record ready for appending.
func NewDiffRecord(txID string, data *DiffData) (*Record, error) {
	raw, err := json.Marshal(data)
	if err != nil {
		return nil, fmt.Errorf("marshal diff data: %w", err)
	}
	return &Record{
		Version:   RecordVersion,
		Type:      RecordTypeDiff,
		Timestamp: Now(),
		TxID:      txID,
		Data:      raw,
	}, nil
}

// NewValidateRecord creates a validate record ready for appending.
func NewValidateRecord(txID string, data *ValidateData) (*Record, error) {
	raw, err := json.Marshal(data)
	if err != nil {
		return nil, fmt.Errorf("marshal validate data: %w", err)
	}
	return &Record{
		Version:   RecordVersion,
		Type:      RecordTypeValidate,
		Timestamp: Now(),
		TxID:      txID,
		Data:      raw,
	}, nil
}

// NewTxRecord creates a transaction lifecycle record ready for appending.
func NewTxRecord(txID string, data *TxData) (*Record, error) {
	raw, err := json.Marshal(data)
	if err != nil {
		return nil, fmt.Errorf("marshal tx data: %w", err)
	}
	return &Record{
		Version:   RecordVersion,
		Type:      RecordTypeTransaction,
		Timestamp: Now(),
		TxID:      txID,
		Data:      raw,
	}, nil
}

// DecodeDiff extracts DiffData from a record's Data field.
func (r *Record) DecodeDiff() (*DiffData, error) {
	if r.Type != RecordTypeDiff {
		return nil, fmt.Errorf("record type is %s, not diff", r.Type)
	}
	var d DiffData
	if err := json.Unmarshal(r.Data, &d); err != nil {
		return nil, fmt.Errorf("decode diff data: %w", err)
	}
	return &d, nil
}

// DecodeValidate extracts ValidateData from a record's Data field.
func (r *Record) DecodeValidate() (*ValidateData, error) {
	if r.Type != RecordTypeValidate {
		return nil, fmt.Errorf("record type is %s, not validate", r.Type)
	}
	var v ValidateData
	if err := json.Unmarshal(r.Data, &v); err != nil {
		return nil, fmt.Errorf("decode validate data: %w", err)
	}
	return &v, nil
}

// DecodeTx extracts TxData from a record's Data field.
func (r *Record) DecodeTx() (*TxData, error) {
	if r.Type != RecordTypeTransaction {
		return nil, fmt.Errorf("record type is %s, not transaction", r.Type)
	}
	var t TxData
	if err := json.Unmarshal(r.Data, &t); err != nil {
		return nil, fmt.Errorf("decode tx data: %w", err)
	}
	return &t, nil
}
