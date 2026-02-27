package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/absorb"
	"github.com/wvandaal/ddis/internal/events"
)

// ddis:implements APP-ADR-024 (bilateral specification)
// ddis:maintains APP-INV-031 (absorbed artifacts validate)
// ddis:maintains APP-INV-032 (symmetric reconciliation)

var (
	absorbAgainst  string
	absorbOutput   string
	absorbPromptOnly bool
	absorbDepth    int
)

var absorbCmd = &cobra.Command{
	Use:   "absorb <code-root>",
	Short: "Absorb code patterns into spec (code-to-spec bridge)",
	Long: `Scans a codebase for patterns (annotations, assertions, error handling,
interfaces) and generates a reconciliation report against an existing spec.

Without --against: lists discovered patterns with suggested spec structure.
With --against: performs bidirectional reconciliation (APP-INV-032):
  - Correspondences: code patterns matching spec elements
  - Undocumented: code behavior not in spec
  - Unimplemented: spec claims with no code evidence

The CLI generates prompts and reports; an external LLM interprets them
to produce draft spec content.

Examples:
  ddis absorb ./src                          # List patterns
  ddis absorb ./src --against index.db       # Reconcile with spec
  ddis absorb ./src --against index.db -o draft.md`,
	Args:          cobra.ExactArgs(1),
	RunE:          runAbsorb,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	absorbCmd.Flags().StringVar(&absorbAgainst, "against", "", "Spec database for bidirectional reconciliation")
	absorbCmd.Flags().StringVar(&absorbOutput, "output", "", "Output path for draft spec (default: stdout)")
	absorbCmd.Flags().BoolVar(&absorbPromptOnly, "prompt-only", false, "Emit prompt without side effects")
	absorbCmd.Flags().IntVar(&absorbDepth, "depth", 0, "Conversation depth for k* budget")
}

func runAbsorb(cmd *cobra.Command, args []string) error {
	opts := absorb.AbsorbOptions{
		CodeRoot:   args[0],
		AgainstDB:  absorbAgainst,
		OutputPath: absorbOutput,
		PromptOnly: absorbPromptOnly,
		Depth:      absorbDepth,
	}

	result, err := absorb.Absorb(opts)
	if err != nil {
		return fmt.Errorf("absorb: %w", err)
	}

	out, err := result.RenderJSON()
	if err != nil {
		return err
	}
	fmt.Println(out)

	// ddis:maintains APP-INV-053 (event stream completeness — emits implementation_finding to stream 3)
	emitEvent(".", events.StreamImplementation, events.TypeImplementationFinding, "", map[string]interface{}{
		"code_root": args[0],
		"command":   "absorb",
	})

	return nil
}
