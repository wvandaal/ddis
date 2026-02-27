package cli

// ddis:implements APP-ADR-038 (Z3 subprocess as Tier 5 — CLI command)
// ddis:implements APP-ADR-042 (Tier 6 LLM-as-judge — CLI invocation)
// ddis:maintains APP-ADR-034 (superseded — gophersat retained for fast propositional path)
// ddis:maintains APP-INV-019 (contradiction graph soundness — user-facing)
// ddis:maintains APP-INV-021 (SAT encoding fidelity — Tier 3+5 invocation)
// ddis:maintains APP-INV-054 (LLM provider graceful degradation — Tier 6 skips when unavailable)

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/consistency"
	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/storage"
)

var (
	contradictTier int
	contradictJSON bool
	contradictZ3   bool
	contradictLLM  bool
)

var contradictCmd = &cobra.Command{
	Use:   "contradict [db-path]",
	Short: "Detect contradictions between spec elements",
	Long: `Runs tiered contradiction detection on the spec index.

Six detection tiers:
  Tier 1 (structural): Existing validation checks (ddis validate)
  Tier 2 (graph):      Cross-reference overlap, governance conflict, neg-spec violations
  Tier 3 (SAT):        Semi-formal → propositional encoding, DPLL satisfiability
  Tier 4 (heuristic):  Polarity inversion, quantifier conflict, numeric bounds, LSI tension
  Tier 5 (SMT):        Semi-formal → SMT-LIB2 via Z3 subprocess (arithmetic, quantifiers)
  Tier 6 (LLM):        Semantic contradiction via LLM-as-judge (Anthropic API, majority vote)

By default runs tiers 2-4. Use --tier 5/--z3 for SMT, --tier 6/--llm for LLM analysis.
Z3 must be installed (apt install z3). ANTHROPIC_API_KEY required for Tier 6.
Graceful degradation when either is absent.

Examples:
  ddis contradict                          # Tiers 2-4, auto-find DB
  ddis contradict manifest.ddis.db         # Explicit DB path
  ddis contradict --tier 2                 # Graph analysis only
  ddis contradict --tier 5                 # All tiers including SMT/Z3
  ddis contradict --z3                     # Shorthand for --tier 5
  ddis contradict --tier 6                 # All tiers including LLM-as-judge
  ddis contradict --llm                    # Shorthand for --tier 6
  ddis contradict --json                   # Machine-readable output`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runContradict,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	contradictCmd.Flags().IntVar(&contradictTier, "tier", 4, "Maximum tier to run (2-6)")
	contradictCmd.Flags().BoolVar(&contradictJSON, "json", false, "JSON output")
	contradictCmd.Flags().BoolVar(&contradictZ3, "z3", false, "Enable Tier 5 SMT/Z3 analysis (shorthand for --tier 5)")
	contradictCmd.Flags().BoolVar(&contradictLLM, "llm", false, "Enable Tier 6 LLM-as-judge analysis (shorthand for --tier 6)")
}

func runContradict(cmd *cobra.Command, args []string) error {
	dbPath := ""
	if len(args) >= 1 {
		dbPath = args[0]
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

	WarnIfStale(db, specID)

	tier := contradictTier
	if contradictZ3 && tier < int(consistency.TierSMT) {
		tier = int(consistency.TierSMT)
	}
	if contradictLLM && tier < int(consistency.TierLLM) {
		tier = int(consistency.TierLLM)
	}
	opts := consistency.Options{
		MaxTier: consistency.Tier(tier),
	}
	if opts.MaxTier < consistency.TierGraph {
		opts.MaxTier = consistency.TierGraph
	}

	result, err := consistency.Analyze(db, specID, opts)
	if err != nil {
		return fmt.Errorf("analyze: %w", err)
	}

	// Emit contradiction_detected event if contradictions found
	if len(result.Contradictions) > 0 {
		tierNames := make([]string, len(result.TiersRun))
		for i, t := range result.TiersRun {
			tierNames[i] = t.String()
		}
		emitEvent(dbPath, events.StreamSpecification, events.TypeContradictionDetected,
			specHashFromDB(db, specID), map[string]interface{}{
				"contradictions": len(result.Contradictions),
				"elements":       result.ElementsScanned,
				"tiers":          tierNames,
			})
	}

	if contradictJSON {
		return renderContradictJSON(result)
	}

	return renderContradictText(result)
}

func renderContradictJSON(result *consistency.Result) error {
	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	return enc.Encode(result)
}

func renderContradictText(result *consistency.Result) error {
	if len(result.Contradictions) == 0 {
		fmt.Printf("No contradictions detected (%d elements scanned, tiers: %s)\n",
			result.ElementsScanned, formatTiers(result.TiersRun))
		if !NoGuidance {
			fmt.Println("\nNext: ddis validate")
			fmt.Println("  No contradictions found — continue with structural validation.")
		}
		return nil
	}

	fmt.Printf("Contradiction Report (%d found, %d elements scanned, tiers: %s)\n",
		len(result.Contradictions), result.ElementsScanned, formatTiers(result.TiersRun))
	fmt.Println(strings.Repeat("═", 60))

	for i, c := range result.Contradictions {
		fmt.Printf("\n%d. [Tier %d/%s] %s ↔ %s  (confidence: %.0f%%)\n",
			i+1, int(c.Tier), c.Type, c.ElementA, c.ElementB, c.Confidence*100)
		fmt.Printf("   %s\n", c.Description)
		fmt.Printf("   Evidence: %s\n", c.Evidence)
		if c.ResolutionHint != "" {
			fmt.Printf("   Hint: %s\n", c.ResolutionHint)
		}
	}

	if !NoGuidance {
		fmt.Printf("\nNext: ddis context %s\n", result.Contradictions[0].ElementA)
		fmt.Println("  Investigate the highest-confidence contradiction.")
	}

	return nil
}

func formatTiers(tiers []consistency.Tier) string {
	names := make([]string, len(tiers))
	for i, t := range tiers {
		names[i] = t.String()
	}
	return strings.Join(names, ", ")
}
