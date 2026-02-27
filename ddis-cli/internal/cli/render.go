package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/renderer"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:maintains APP-INV-001 (round-trip fidelity)

var (
	renderOutput string
	renderFormat string
)

var renderCmd = &cobra.Command{
	Use:   "render [index.db]",
	Short: "Render an index back to markdown",
	Args:  cobra.RangeArgs(0, 1),
	RunE:  runRender,
}

func init() {
	renderCmd.Flags().StringVarP(&renderOutput, "output", "o", "", "Output file or directory path")
	renderCmd.Flags().StringVarP(&renderFormat, "format", "f", "monolith", "Output format: monolith or modular")
}

func runRender(cmd *cobra.Command, args []string) error {
	var dbPath string
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

	// Get the first (and typically only) spec ID
	var specID int64
	if err := db.QueryRow("SELECT id FROM spec_index LIMIT 1").Scan(&specID); err != nil {
		return fmt.Errorf("no spec found in database: %w", err)
	}

	output := renderOutput
	if output == "" {
		if renderFormat == "modular" {
			output = "output/"
		} else {
			output = "output.md"
		}
	}

	switch renderFormat {
	case "monolith":
		fmt.Printf("Rendering monolith to %s...\n", output)
		if err := renderer.RenderMonolith(db, specID, output); err != nil {
			return err
		}
	case "modular":
		fmt.Printf("Rendering modular to %s...\n", output)
		if err := renderer.RenderModular(db, specID, output); err != nil {
			return err
		}
	default:
		return fmt.Errorf("unknown format: %s (use 'monolith' or 'modular')", renderFormat)
	}

	// ddis:maintains APP-INV-053 (event stream completeness — emits artifact_written to stream 1)
	emitEvent(dbPath, events.StreamDiscovery, events.TypeArtifactWritten, specHashFromDB(db, specID), map[string]interface{}{
		"output_path": output,
		"format":      renderFormat,
		"command":     "render",
	})

	fmt.Println("Done.")
	return nil
}
