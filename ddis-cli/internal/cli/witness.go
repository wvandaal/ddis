package cli

// ddis:implements APP-INV-041 (witness auto-invalidation)
// ddis:implements APP-ADR-030 (persistent witnesses over ephemeral done flags)
// ddis:maintains APP-INV-054 (LLM provider graceful degradation — eval type skips when provider unavailable)
// ddis:maintains APP-INV-055 (eval evidence statistical soundness — wires --type eval to majority vote)

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"os"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/witness"
)

var (
	witnessType          string
	witnessEvidence      string
	witnessBy            string
	witnessModel         string
	witnessCodeHash      string
	witnessCodeRoot      string
	witnessVerify        bool
	witnessReviewContext bool
	witnessNotes         string
	witnessList          bool
	witnessCheck         bool
	witnessRevoke        string
	witnessJSON          bool
)

var witnessCmd = &cobra.Command{
	Use:   "witness [INV-ID] [db-path]",
	Short: "Record, check, or revoke invariant witnesses",
	Long: `Manages proof receipts (witnesses) for implemented invariants.

Witnesses record evidence that an invariant is implemented, along with a hash
of the invariant's current definition. If the spec changes, the witness
auto-invalidates. This turns 'ddis progress' from a snapshot calculator into
a live dashboard of proven properties.

Four evidence levels:
  Level 1 (attestation):  Agent declares "I implemented this"
  Level 2 (test/annotation): Agent provides proof string
  Level 3 (scan/--verify): Mechanical check that code annotations exist
  Level 4 (review):       Gestalt-Theory-framed code review

Examples:
  ddis witness APP-INV-001 db.db --by agent-123             # Level 1: attestation
  ddis witness APP-INV-001 db.db --type test --evidence "TestRoundTrip passes" --by agent-123
  ddis witness APP-INV-001 db.db --verify --code-root . --by agent-123 --model claude-opus-4-6
  ddis witness APP-INV-001 db.db --review-context --code-root . --json
  ddis witness APP-INV-001 db.db --type review --evidence '{"verdict":"faithful"}' --by agent-123
  ddis witness --list db.db --json
  ddis witness --check db.db --json
  ddis witness --revoke APP-INV-001 db.db`,
	Args:          cobra.MaximumNArgs(2),
	RunE:          runWitness,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	witnessCmd.Flags().StringVar(&witnessType, "type", "attestation", "Evidence type: attestation, test, annotation, scan, review, eval")
	witnessCmd.Flags().StringVar(&witnessEvidence, "evidence", "", "Proof content (free text or JSON)")
	witnessCmd.Flags().StringVar(&witnessBy, "by", "", "Agent session/agent ID (traceable)")
	witnessCmd.Flags().StringVar(&witnessModel, "model", "", "Model type (e.g., claude-opus-4-6)")
	witnessCmd.Flags().StringVar(&witnessCodeHash, "code-hash", "", "Optional code hash")
	witnessCmd.Flags().StringVar(&witnessCodeRoot, "code-root", "", "Path to code root for --verify/--review-context")
	witnessCmd.Flags().BoolVar(&witnessVerify, "verify", false, "Require mechanical annotation proof (Level 3)")
	witnessCmd.Flags().BoolVar(&witnessReviewContext, "review-context", false, "Output Gestalt-framed review bundle (Level 4 phase A)")
	witnessCmd.Flags().StringVar(&witnessNotes, "notes", "", "Optional notes")
	witnessCmd.Flags().BoolVar(&witnessList, "list", false, "List all witnesses")
	witnessCmd.Flags().BoolVar(&witnessCheck, "check", false, "Check witness freshness")
	witnessCmd.Flags().StringVar(&witnessRevoke, "revoke", "", "Revoke witness for invariant ID")
	witnessCmd.Flags().BoolVar(&witnessJSON, "json", false, "Output as JSON")
}

func runWitness(cmd *cobra.Command, args []string) error {
	var dbPath string
	var invariantID string

	if witnessList || witnessCheck || witnessRevoke != "" {
		if len(args) >= 1 {
			dbPath = args[0]
		}
		if witnessRevoke != "" {
			invariantID = witnessRevoke
		}
	} else {
		if len(args) >= 1 {
			invariantID = args[0]
		}
		if len(args) >= 2 {
			dbPath = args[1]
		}
	}

	if dbPath == "" {
		var err error
		dbPath, err = FindDB()
		if err != nil {
			return err
		}
	}

	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	switch {
	case witnessList:
		return witnessListMode(db, specID)
	case witnessCheck:
		return witnessCheckMode(db, specID)
	case witnessRevoke != "":
		return witnessRevokeMode(db, specID, invariantID)
	case witnessReviewContext:
		return witnessReviewMode(db, specID, invariantID)
	default:
		return witnessRecordMode(db, specID, invariantID)
	}
}

func witnessListMode(db *sql.DB, specID int64) error {
	witnesses, err := storage.ListWitnesses(db, specID)
	if err != nil {
		return err
	}
	out, err := witness.RenderList(witnesses, witnessJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)
	return nil
}

func witnessCheckMode(db *sql.DB, specID int64) error {
	staleCount, err := witness.Refresh(db, specID)
	if err != nil {
		return err
	}
	if staleCount > 0 {
		fmt.Fprintf(os.Stderr, "Refreshed: %d witness(es) marked stale\n", staleCount)
	}

	summary, err := witness.Check(db, specID, witness.CheckOptions{AsJSON: witnessJSON})
	if err != nil {
		return err
	}
	out, err := witness.Render(summary, witnessJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)
	return nil
}

func witnessRevokeMode(db *sql.DB, specID int64, invariantID string) error {
	if invariantID == "" {
		return fmt.Errorf("--revoke requires an invariant ID")
	}
	if err := storage.DeleteWitness(db, specID, invariantID); err != nil {
		return err
	}
	fmt.Printf("Witness revoked: %s\n", invariantID)
	return nil
}

func witnessReviewMode(db *sql.DB, specID int64, invariantID string) error {
	if invariantID == "" {
		return fmt.Errorf("review-context requires an invariant ID")
	}
	codeRoot := witnessCodeRoot
	if codeRoot == "" {
		codeRoot = "."
	}
	bundle, err := witness.BuildReviewContext(db, specID, invariantID, codeRoot)
	if err != nil {
		return err
	}
	data, err := json.MarshalIndent(bundle, "", "  ")
	if err != nil {
		return fmt.Errorf("marshal review context: %w", err)
	}
	fmt.Println(string(data))
	return nil
}

func witnessRecordMode(db *sql.DB, specID int64, invariantID string) error {
	if invariantID == "" {
		return fmt.Errorf("witness requires an invariant ID (e.g., ddis witness APP-INV-001 db.db)")
	}

	// Eval type uses majority-vote LLM evaluation.
	if witnessType == "eval" {
		evalOpts := witness.EvalOptions{
			InvariantID: invariantID,
			ProvenBy:    witnessBy,
			CodeRoot:    witnessCodeRoot,
			Notes:       witnessNotes,
			AsJSON:      witnessJSON,
		}
		result, err := witness.RecordEval(db, specID, evalOpts)
		if err != nil {
			return err
		}
		if witnessJSON {
			data, _ := json.MarshalIndent(result, "", "  ")
			fmt.Println(string(data))
		} else {
			fmt.Printf("Eval witness: %s — verdict=%s confidence=%.2f (%d/%d agreement, model=%s)\n",
				result.InvariantID, result.Verdict, result.Confidence,
				result.Agreement, result.Runs, result.ModelID)
		}
		if !NoGuidance && !witnessJSON {
			fmt.Println("\nNext: ddis challenge", invariantID)
			fmt.Println("  Challenge the invariant to verify the eval witness.")
		}
		return nil
	}

	opts := witness.Options{
		InvariantID:  invariantID,
		EvidenceType: witnessType,
		Evidence:     witnessEvidence,
		ProvenBy:     witnessBy,
		Model:        witnessModel,
		CodeHash:     witnessCodeHash,
		CodeRoot:     witnessCodeRoot,
		Verify:       witnessVerify,
		Notes:        witnessNotes,
		AsJSON:       witnessJSON,
	}

	w, err := witness.Record(db, specID, opts)
	if err != nil {
		return err
	}

	out, err := witness.RenderSingle(w, witnessJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)

	if !NoGuidance && !witnessJSON {
		fmt.Println("\nNext: ddis progress")
		fmt.Println("  See updated implementation status with witnesses.")
	}

	return nil
}
