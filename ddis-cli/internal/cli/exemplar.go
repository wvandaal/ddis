package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/exemplar"
	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

var (
	exemplarJSON     bool
	exemplarGap      string
	exemplarLimit    int
	exemplarMinScore float64
	exemplarCorpus   []string
)

var exemplarCmd = &cobra.Command{
	Use:   "exemplar <target> [index.db]",
	Short: "Find corpus-derived demonstrations for missing or weak spec components",
	Long: `Analyzes a spec element for component gaps and finds the best existing
examples from the corpus to demonstrate what strong content looks like.

Transforms the CLI from "here's what's wrong" to "here's what good looks like"
by leveraging LLM Gestalt Theory — demonstrations activate knowledge substrates
5-20x more efficiently than constraints.

Examples:
  ddis exemplar APP-INV-006 index.db
  ddis exemplar APP-INV-006 index.db --json
  ddis exemplar APP-INV-006 index.db --gap violation_scenario
  ddis exemplar APP-ADR-003 index.db --limit 5
  ddis exemplar INV-006 index.db --min-score 0.5
  ddis exemplar INV-006 index.db --corpus other.ddis.db`,
	Args:          cobra.RangeArgs(1, 2),
	RunE:          runExemplar,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	exemplarCmd.Flags().BoolVar(&exemplarJSON, "json", false, "Output as JSON (optimized for LLM consumption)")
	exemplarCmd.Flags().StringVar(&exemplarGap, "gap", "", "Focus on specific component (e.g., violation_scenario, problem)")
	exemplarCmd.Flags().IntVar(&exemplarLimit, "limit", 3, "Max exemplars per gap")
	exemplarCmd.Flags().Float64Var(&exemplarMinScore, "min-score", 0.3, "Quality threshold for exemplars")
	exemplarCmd.Flags().StringSliceVar(&exemplarCorpus, "corpus", nil, "Additional .ddis.db paths for cross-spec exemplars")
}

func runExemplar(cmd *cobra.Command, args []string) error {
	target := args[0]

	var dbPath string
	if len(args) >= 2 {
		dbPath = args[1]
	}
	if dbPath == "" {
		var err error
		dbPath, err = findDB()
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
		return fmt.Errorf("no spec found in database: %w", err)
	}

	// Build LSI index for similarity computation
	docs, err := search.ExtractDocuments(db, specID)
	if err != nil {
		return fmt.Errorf("extract documents: %w", err)
	}

	k := 50
	if len(docs) < k {
		k = len(docs)
	}
	lsi, err := search.BuildLSI(docs, k)
	if err != nil {
		// LSI failure is non-fatal
		lsi = nil
	}

	opts := exemplar.Options{
		Target:   target,
		Gap:      exemplarGap,
		Limit:    exemplarLimit,
		MinScore: exemplarMinScore,
		AsJSON:   exemplarJSON,
		Corpus:   exemplarCorpus,
	}

	result, err := exemplar.Analyze(db, specID, lsi, opts)
	if err != nil {
		return err
	}

	out, err := exemplar.Render(result, exemplarJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)
	return nil
}
