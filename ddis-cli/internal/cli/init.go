package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/workspace"
)

// ddis:implements APP-ADR-026 (full workspace init)
// ddis:maintains APP-INV-037 (workspace isolation)

var (
	initWorkspace     bool
	initSkeletonLevel int
	initSpecName      string
	initJSON          bool
)

var initCmd = &cobra.Command{
	Use:   "init [directory]",
	Short: "Initialize a new DDIS specification workspace",
	Long: `Creates a complete DDIS workspace with manifest template, constitution
skeleton, SQLite database, JSONL event streams, and .gitignore entries.

Running init in an existing workspace is safe — only missing files are created.
Existing files are never overwritten (idempotent).

Examples:
  ddis init                         # Initialize in current directory
  ddis init ./my-spec               # Initialize in specified directory
  ddis init --name "My System"      # Set spec name
  ddis init --skeleton 3            # Full Level 3 skeleton
  ddis init --workspace             # Multi-spec workspace mode`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runInit,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	initCmd.Flags().BoolVar(&initWorkspace, "workspace", false, "Create workspace.yaml for multi-spec management")
	initCmd.Flags().IntVar(&initSkeletonLevel, "skeleton", 1, "Template maturity level (1-3)")
	initCmd.Flags().StringVar(&initSpecName, "name", "", "Spec name (default: directory name)")
	initCmd.Flags().BoolVar(&initJSON, "json", false, "Output as JSON")
}

func runInit(cmd *cobra.Command, args []string) error {
	root := "."
	if len(args) > 0 {
		root = args[0]
	}

	opts := workspace.InitOptions{
		Root:          root,
		Workspace:     initWorkspace,
		SkeletonLevel: initSkeletonLevel,
		SpecName:      initSpecName,
	}

	result, err := workspace.Init(opts)
	if err != nil {
		return fmt.Errorf("init: %w", err)
	}

	if initJSON {
		out, err := workspace.RenderJSON(result)
		if err != nil {
			return err
		}
		fmt.Println(out)
	} else {
		fmt.Print(workspace.RenderText(result))
	}

	// ddis:maintains APP-INV-053 (event stream completeness — emits artifact_written to stream 1)
	emitEvent(root, events.StreamDiscovery, events.TypeArtifactWritten, "", map[string]interface{}{
		"root":    root,
		"command": "init",
	})

	return nil
}
