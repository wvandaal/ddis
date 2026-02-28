package cli

// ddis:implements APP-INV-050 (challenge-witness adjunction fidelity)
// ddis:implements APP-ADR-037 (challenge as right adjoint of witness)
// ddis:maintains APP-INV-066 (recursive feedback — refutation suggests filing remediation issue)

import (
	"database/sql"
	"fmt"

	"github.com/spf13/cobra"
	"github.com/wvandaal/ddis/internal/challenge"
	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/storage"
)

var (
	challengeCodeRoot string
	challengeJSON     bool
	challengeAll      bool
	challengeMaxLevel int
	challengeBy       string
	challengeModel    string
)

var challengeCmd = &cobra.Command{
	Use:   "challenge [INV-ID] [db-path]",
	Short: "Mechanically verify witness claims (right adjoint of witness)",
	Long: `Challenges witness claims by running 5-level mechanical verification:

  Level 1 (Formal):      SAT consistency of semi-formal expression
  Level 2 (Uncertainty): Evidence type confidence scoring
  Level 3 (Causal):      Annotation lookup (ddis:tests INV-ID)
  Level 4 (Practical):   Execute the referenced test
  Level 5 (Meta):        Keyword overlap between invariant and evidence

Verdicts:
  confirmed:    All applicable levels pass
  refuted:      Test failed or semi-formal is self-contradictory
  inconclusive: Missing annotations, low confidence, or can't run test

On refutation, the witness is automatically invalidated.

Examples:
  ddis challenge APP-INV-001 db.db --code-root .
  ddis challenge --all db.db --code-root . --json
  ddis challenge APP-INV-001 --max-level 3           # Skip test execution`,
	Args:          cobra.MaximumNArgs(2),
	RunE:          runChallenge,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	challengeCmd.Flags().StringVar(&challengeCodeRoot, "code-root", "", "Path to code root for annotation/test lookup")
	challengeCmd.Flags().BoolVar(&challengeJSON, "json", false, "Output as JSON")
	challengeCmd.Flags().BoolVar(&challengeAll, "all", false, "Challenge all valid witnesses")
	challengeCmd.Flags().IntVar(&challengeMaxLevel, "max-level", 5, "Maximum verification level (1-5)")
	challengeCmd.Flags().StringVar(&challengeBy, "by", "challenge-agent", "Challenger identity")
	challengeCmd.Flags().StringVar(&challengeModel, "model", "", "Model used for challenge")
}

func runChallenge(cmd *cobra.Command, args []string) error {
	var dbPath string
	var invariantID string

	if challengeAll {
		if len(args) >= 1 {
			dbPath = args[0]
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

	db, err := storage.OpenExisting(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	opts := challenge.Options{
		CodeRoot:     challengeCodeRoot,
		ChallengedBy: challengeBy,
		Model:        challengeModel,
		AsJSON:       challengeJSON,
		MaxLevel:     challengeMaxLevel,
	}

	if challengeAll {
		return runChallengeAll(db, dbPath, specID, opts)
	}

	if invariantID == "" {
		return fmt.Errorf("invariant ID required (or use --all)")
	}

	return runChallengeSingle(db, dbPath, specID, invariantID, opts)
}

func runChallengeSingle(db *sql.DB, dbPath string, specID int64, invariantID string, opts challenge.Options) error {
	result, err := challenge.Challenge(db, specID, invariantID, opts)
	if err != nil {
		return err
	}

	output, err := challenge.Render(result, opts.AsJSON)
	if err != nil {
		return err
	}
	fmt.Println(output)

	// Emit events
	specHash := specHashFromDB(db, specID)
	emitEvent(dbPath, events.StreamImplementation, events.TypeChallengeIssued, specHash, map[string]interface{}{
		"invariant_id": invariantID,
		"verdict":      result.Verdict,
	})
	// ddis:implements APP-INV-072 (event content completeness — challenge emits structured payload)
	emitEvent(dbPath, events.StreamImplementation, events.TypeChallengeCompleted, specHash, events.ChallengePayload{
		InvariantID: invariantID,
		Verdict:     string(result.Verdict),
		Score:       result.EvidenceScore,
	})

	// Guidance postscript
	if !NoGuidance {
		switch result.Verdict {
		case challenge.Confirmed:
			fmt.Println("\nNext: ddis progress")
			fmt.Println("  Witness confirmed — check overall implementation progress.")
		case challenge.Provisional:
			fmt.Printf("\nNext: Add // ddis:tests %s annotation above a test function\n", invariantID)
			fmt.Println("  Provisionally confirmed via code annotations. Strengthen to full confirmation")
			fmt.Println("  by adding a ddis:tests annotation and re-challenging.")
		case challenge.Refuted:
			fmt.Printf("\nNext: ddis issue file \"Remediate %s\" --label refutation\n", invariantID)
			fmt.Println("  Witness invalidated — file a remediation issue for tracked resolution.")
			fmt.Printf("  Then: ddis witness %s --type test --evidence \"...\"\n", invariantID)
		case challenge.Inconclusive:
			if result.LevelUncertainty != nil && result.LevelUncertainty.Confidence <= 0.3 {
				fmt.Printf("\nNext: ddis witness %s --type test --evidence \"...\"\n", invariantID)
				fmt.Println("  Attestation-only evidence (confidence=0.3). Upgrade to test/scan/review.")
			} else if opts.MaxLevel < 5 {
				fmt.Printf("\nNext: ddis challenge %s --code-root . --max-level %d\n", invariantID, opts.MaxLevel+1)
				fmt.Println("  Challenge inconclusive — try higher verification level.")
			} else {
				fmt.Printf("\nNext: ddis witness %s --verify --code-root .\n", invariantID)
				fmt.Println("  Challenge inconclusive — add annotations or strengthen evidence.")
			}
		}
	}

	return nil
}

func runChallengeAll(db *sql.DB, dbPath string, specID int64, opts challenge.Options) error {
	results, err := challenge.ChallengeAll(db, specID, opts)
	if err != nil {
		return err
	}

	if len(results) == 0 {
		fmt.Println("No valid witnesses to challenge.")
		if !NoGuidance {
			fmt.Println("\nNext: ddis witness --list")
		}
		return nil
	}

	output, err := challenge.RenderAll(results, opts.AsJSON)
	if err != nil {
		return err
	}
	fmt.Println(output)

	// Emit event
	specHash := specHashFromDB(db, specID)
	confirmed, provisional, refuted, inconclusive := 0, 0, 0, 0
	for _, r := range results {
		switch r.Verdict {
		case challenge.Confirmed:
			confirmed++
		case challenge.Provisional:
			provisional++
		case challenge.Refuted:
			refuted++
		case challenge.Inconclusive:
			inconclusive++
		}
	}
	emitEvent(dbPath, events.StreamImplementation, events.TypeChallengeBatch, specHash, map[string]interface{}{
		"total":        len(results),
		"confirmed":    confirmed,
		"provisional":  provisional,
		"refuted":      refuted,
		"inconclusive": inconclusive,
	})
	// ddis:implements APP-INV-072 (event content completeness — challenge batch emits per-result structured payloads)
	for _, r := range results {
		emitEvent(dbPath, events.StreamImplementation, events.TypeChallengeCompleted, specHash, events.ChallengePayload{
			InvariantID: r.InvariantID,
			Verdict:     string(r.Verdict),
			Score:       r.EvidenceScore,
		})
	}

	if !NoGuidance {
		if refuted > 0 {
			fmt.Printf("\nNext: ddis issue file \"Remediate refuted invariants\" --label refutation\n")
			fmt.Printf("  %d witness(es) refuted — file remediation issue(s) for tracked resolution.\n", refuted)
		} else if inconclusive > 0 {
			fmt.Printf("\nNext: Strengthen %d inconclusive witnesses (attestation-only → test/scan evidence)\n", inconclusive)
			fmt.Println("  Run: ddis witness INV-ID --type test --evidence \"...\"")
		} else {
			fmt.Println("\nNext: ddis progress")
			fmt.Println("  All challenges resolved — check implementation progress.")
		}
	}

	return nil
}
