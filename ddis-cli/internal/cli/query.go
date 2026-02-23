package cli

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/query"
	"github.com/wvandaal/ddis/internal/storage"
)

var (
	queryJSON            bool
	queryRaw             bool
	queryResolveRefs     bool
	queryIncludeGlossary bool
	queryBacklinks       bool
	queryList            string
	queryStats           bool
)

var queryCmd = &cobra.Command{
	Use:   "query <target|--list TYPE|--stats> [index.db]",
	Short: "Query the spec index for fragments, lists, or stats",
	Long: `Retrieve spec fragments by target (§N.M, INV-NNN, ADR-NNN, Gate-N),
list elements by type, or show index statistics.

Examples:
  ddis query INV-006 index.db
  ddis query §0.5 index.db --resolve-refs --include-glossary
  ddis query ADR-003 index.db --backlinks --json
  ddis query --list invariants index.db
  ddis query --stats index.db`,
	Args: cobra.RangeArgs(0, 2),
	RunE: runQuery,
}

func init() {
	queryCmd.Flags().BoolVar(&queryJSON, "json", false, "Output as JSON")
	queryCmd.Flags().BoolVar(&queryRaw, "raw", false, "Output raw text only")
	queryCmd.Flags().BoolVar(&queryResolveRefs, "resolve-refs", false, "Follow and include outgoing cross-references")
	queryCmd.Flags().BoolVar(&queryIncludeGlossary, "include-glossary", false, "Include matching glossary definitions")
	queryCmd.Flags().BoolVar(&queryBacklinks, "backlinks", false, "Show what references the target")
	queryCmd.Flags().StringVar(&queryList, "list", "", "List all items of TYPE (invariants, adrs, gates, sections, glossary, modules)")
	queryCmd.Flags().BoolVar(&queryStats, "stats", false, "Show index statistics")
}

func runQuery(cmd *cobra.Command, args []string) error {
	// Determine mode and DB path
	var dbPath, target string

	if queryStats || queryList != "" {
		// --stats or --list: first positional arg (if any) is the DB
		if len(args) >= 1 {
			dbPath = args[0]
		}
	} else {
		// Normal query: first arg is target, optional second is DB
		if len(args) < 1 {
			return fmt.Errorf("target required: specify §N.M, INV-NNN, ADR-NNN, or Gate-N")
		}
		target = args[0]
		if len(args) >= 2 {
			dbPath = args[1]
		}
	}

	// Auto-discover DB if not specified
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

	// Stats mode
	if queryStats {
		stats, err := query.ComputeStats(db, specID)
		if err != nil {
			return err
		}
		out, err := query.RenderStats(stats, queryJSON)
		if err != nil {
			return err
		}
		fmt.Print(out)
		return nil
	}

	// List mode
	if queryList != "" {
		listType, err := query.ParseListType(queryList)
		if err != nil {
			return err
		}
		items, err := query.ListElements(db, specID, listType)
		if err != nil {
			return err
		}
		out, err := query.RenderList(items, listType, queryJSON)
		if err != nil {
			return err
		}
		fmt.Print(out)
		return nil
	}

	// Fragment query
	opts := query.QueryOptions{
		ResolveRefs:     queryResolveRefs,
		IncludeGlossary: queryIncludeGlossary,
		Backlinks:       queryBacklinks,
	}

	frag, err := query.QueryTarget(db, specID, target, opts)
	if err != nil {
		return err
	}

	format := query.FormatMarkdown
	if queryJSON {
		format = query.FormatJSON
	} else if queryRaw {
		format = query.FormatRaw
	}

	out, err := query.RenderFragment(frag, format)
	if err != nil {
		return err
	}
	fmt.Print(out)
	return nil
}

// findDB looks for a *.ddis.db file in the current directory.
func findDB() (string, error) {
	matches, err := filepath.Glob("*.ddis.db")
	if err != nil {
		return "", fmt.Errorf("search for .ddis.db: %w", err)
	}
	if len(matches) == 0 {
		return "", fmt.Errorf("no .ddis.db file found in current directory; specify the database path explicitly")
	}
	if len(matches) > 1 {
		return "", fmt.Errorf("multiple .ddis.db files found (%s); specify which one to use", strings.Join(matches, ", "))
	}
	return matches[0], nil
}
