package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-005 (context self-containment)
// ddis:implements APP-ADR-006 (bundle intelligence)

var (
	contextJSON         bool
	contextOplogPath    string
	contextDepth        int
	contextRelatedLimit int
)

var contextCmd = &cobra.Command{
	Use:   "context <target> [index.db]",
	Short: "Generate a contextual intelligence bundle for a spec element",
	Long: `Builds a pre-flight briefing for a spec element by orchestrating search,
impact analysis, validation, and oplog into a single token-optimized bundle.
Designed for LLM consumption — one command replaces 5-10 separate queries.

Examples:
  ddis context §4.2 index.db
  ddis context INV-006 index.db --json
  ddis context Gate-1 index.db --depth 3
  ddis context ADR-003 index.db --oplog-path log.jsonl`,
	Args:          cobra.RangeArgs(1, 2),
	RunE:          runContext,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	contextCmd.Flags().BoolVar(&contextJSON, "json", false, "Output as JSON (optimized for LLM consumption)")
	contextCmd.Flags().StringVar(&contextOplogPath, "oplog-path", "", "Path to oplog.jsonl for recent changes")
	contextCmd.Flags().IntVar(&contextDepth, "depth", 2, "Impact analysis depth (1-5)")
	contextCmd.Flags().IntVar(&contextRelatedLimit, "related-limit", 5, "Maximum number of LSI-related elements")
}

func runContext(cmd *cobra.Command, args []string) error {
	target := args[0]

	var dbPath string
	if len(args) >= 2 {
		dbPath = args[1]
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
		// LSI failure is non-fatal — context bundle works without similarity
		lsi = nil
	}

	bundle, err := search.BuildContext(db, specID, target, lsi, contextOplogPath, contextDepth, contextRelatedLimit)
	if err != nil {
		return err
	}

	out, err := search.RenderContext(bundle, contextJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)

	if !NoGuidance && !contextJSON {
		fmt.Printf("\nNext: ddis exemplar %s\n", target)
		fmt.Println("  See corpus demonstrations for weak components.")
	}
	return nil
}
