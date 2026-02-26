package cli

// ddis:implements APP-ADR-034 (pure-Go tiered consistency — CLI command)
// ddis:maintains APP-INV-019 (contradiction graph soundness — user-facing)
// ddis:maintains APP-INV-021 (SAT encoding fidelity — Tier 3 invocation)

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"


	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/consistency"
	"github.com/wvandaal/ddis/internal/storage"
)

var (
	contradictTier int
	contradictJSON bool
)

var contradictCmd = &cobra.Command{
	Use:   "contradict [db-path]",
	Short: "Detect contradictions between spec elements",
	Long: `Runs tiered contradiction detection on the spec index.

Four detection tiers:
  Tier 1 (structural): Existing validation checks (ddis validate)
  Tier 2 (graph):      Cross-reference overlap, governance conflict, neg-spec violations
  Tier 3 (SAT):        Semi-formal → propositional encoding, DPLL satisfiability
  Tier 4 (heuristic):  Polarity inversion, quantifier conflict, numeric bounds, LSI tension

By default runs all tiers. Use --tier to limit.

Examples:
  ddis contradict                          # All tiers, auto-find DB
  ddis contradict manifest.ddis.db         # Explicit DB path
  ddis contradict --tier 2                 # Graph analysis only
  ddis contradict --json                   # Machine-readable output`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runContradict,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	contradictCmd.Flags().IntVar(&contradictTier, "tier", 4, "Maximum tier to run (2-4)")
	contradictCmd.Flags().BoolVar(&contradictJSON, "json", false, "JSON output")
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

	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	WarnIfStale(db, specID)

	opts := consistency.Options{
		MaxTier: consistency.Tier(contradictTier),
	}
	if opts.MaxTier < consistency.TierGraph {
		opts.MaxTier = consistency.TierGraph
	}

	result, err := consistency.Analyze(db, specID, opts)
	if err != nil {
		return fmt.Errorf("analyze: %w", err)
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
