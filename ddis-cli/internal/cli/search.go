package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-008 (RRF fusion correctness)
// ddis:maintains APP-INV-004 (authority monotonicity)
// ddis:implements APP-ADR-003 (hybrid search)

var (
	searchJSON        bool
	searchType        string
	searchLimit       int
	searchSnippets    bool
	searchLexicalOnly bool
)

var searchCmd = &cobra.Command{
	Use:   "search <query> [index.db]",
	Short: "Hybrid search over the spec index (BM25 + LSI + PageRank)",
	Long: `Search the DDIS spec index using a multi-signal hybrid search engine.
Combines BM25 lexical matching, LSI semantic similarity, and PageRank
authority scores via Reciprocal Rank Fusion (RRF).

Examples:
  ddis search "cross-reference density" index.db
  ddis search "INV-006" index.db
  ddis search "LLM consumption" index.db --type invariant --limit 5
  ddis search "validation method" index.db --json
  ddis search "state machine" index.db --snippets
  ddis search "how to verify" index.db --lexical-only`,
	Args:          cobra.RangeArgs(1, 2),
	RunE:          runSearch,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	searchCmd.Flags().BoolVar(&searchJSON, "json", false, "Output as JSON")
	searchCmd.Flags().StringVar(&searchType, "type", "", "Filter by element type (section, invariant, adr, gate, glossary, negative_spec)")
	searchCmd.Flags().IntVar(&searchLimit, "limit", 10, "Maximum number of results")
	searchCmd.Flags().BoolVar(&searchSnippets, "snippets", false, "Include text snippets in results")
	searchCmd.Flags().BoolVar(&searchLexicalOnly, "lexical-only", false, "Use BM25 only (skip LSI semantic matching)")
}

func runSearch(cmd *cobra.Command, args []string) error {
	queryStr := args[0]

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

	results, err := search.Search(db, specID, queryStr, search.SearchOptions{
		Limit:           searchLimit,
		TypeFilter:      searchType,
		LexicalOnly:     searchLexicalOnly,
		IncludeSnippets: searchSnippets,
	})
	if err != nil {
		return err
	}

	out, err := search.RenderSearch(results, searchJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)
	return nil
}
