package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/annotate"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-017 (annotation portability)
// ddis:maintains APP-INV-018 (scan-spec correspondence)
// ddis:implements APP-ADR-012 (annotations over code manifest)

var (
	scanSpec   string
	scanVerify bool
	scanStore  bool
	scanJSON   bool
)

var scanCmd = &cobra.Command{
	Use:   "scan <code-root>",
	Short: "Scan source code for ddis: annotations",
	Long: `Walks a directory tree and extracts all DDIS annotations from source code
comments. Annotations use the grammar: <comment-marker> ddis:<verb> <target>

Supported verbs: maintains, implements, interfaces, tests, validates-via,
postcondition, relates-to, satisfies.

With --spec and --verify, checks annotations against the spec database:
  - Orphaned: annotations targeting non-existent spec elements
  - Unimplemented: spec elements with no code annotations

Examples:
  ddis scan ./src                              # Scan and list annotations
  ddis scan ./src --spec index.db --verify     # Verify against spec
  ddis scan ./src --spec index.db --store      # Store in spec DB
  ddis scan ./src --json                       # Machine-readable output`,
	Args:          cobra.ExactArgs(1),
	RunE:          runScan,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	scanCmd.Flags().StringVar(&scanSpec, "spec", "", "Path to spec database for verification")
	scanCmd.Flags().BoolVar(&scanVerify, "verify", false, "Verify annotations against spec DB (requires --spec)")
	scanCmd.Flags().BoolVar(&scanStore, "store", false, "Store annotations in spec DB (requires --spec)")
	scanCmd.Flags().BoolVar(&scanJSON, "json", false, "Output as JSON")
}

func runScan(cmd *cobra.Command, args []string) error {
	opts := annotate.ScanOptions{
		Root:   args[0],
		SpecDB: scanSpec,
		Verify: scanVerify,
		Store:  scanStore,
		AsJSON: scanJSON,
	}

	result, err := annotate.Scan(opts)
	if err != nil {
		return fmt.Errorf("scan: %w", err)
	}

	// Verify and/or store require a spec DB
	if (opts.Verify || opts.Store) && opts.SpecDB != "" {
		db, err := storage.Open(opts.SpecDB)
		if err != nil {
			return fmt.Errorf("open spec database: %w", err)
		}
		defer db.Close()

		specID, err := storage.GetFirstSpecID(db)
		if err != nil {
			return fmt.Errorf("no spec found: %w", err)
		}

		if opts.Verify {
			if err := annotate.Verify(result, db, specID); err != nil {
				return fmt.Errorf("verify: %w", err)
			}
		}

		if opts.Store {
			if err := annotate.StoreAnnotations(db, specID, result.Annotations); err != nil {
				return fmt.Errorf("store annotations: %w", err)
			}
		}
	}

	if opts.AsJSON {
		out, err := annotate.RenderJSON(result)
		if err != nil {
			return err
		}
		fmt.Println(out)
	} else {
		fmt.Print(annotate.RenderText(result))
	}

	return nil
}
