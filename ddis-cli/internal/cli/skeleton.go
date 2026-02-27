package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/skeleton"
)

// ddis:interfaces APP-INV-001 (round-trip fidelity)

var (
	skeletonName    string
	skeletonDomains []string
	skeletonOutput  string
)

var skeletonCmd = &cobra.Command{
	Use:   "skeleton",
	Short: "Generate a DDIS specification skeleton",
	Long: `Creates a DDIS-conformant specification scaffold with manifest.yaml,
constitution/system.md, and module files. The generated skeleton passes
Gate-1 structural checks and can be filled in following the TODO markers.

Examples:
  ddis skeleton --name "My Spec" --domain core -o ./my-spec
  ddis skeleton --name "Auth System" --domain auth --domain users -o ./auth-spec`,
	RunE:          runSkeleton,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	skeletonCmd.Flags().StringVar(&skeletonName, "name", "", "Specification name (required)")
	skeletonCmd.Flags().StringSliceVar(&skeletonDomains, "domain", nil, "Domain names (at least one required)")
	skeletonCmd.Flags().StringVarP(&skeletonOutput, "output", "o", "", "Output directory (required)")
	skeletonCmd.MarkFlagRequired("name")
	skeletonCmd.MarkFlagRequired("domain")
	skeletonCmd.MarkFlagRequired("output")
}

func runSkeleton(cmd *cobra.Command, args []string) error {
	opts := skeleton.Options{
		Name:    skeletonName,
		Domains: skeletonDomains,
		Output:  skeletonOutput,
	}
	result, err := skeleton.Generate(opts)
	if err != nil {
		return err
	}
	fmt.Printf("Created DDIS spec skeleton:\n  %s/\n", result.OutputDir)
	for _, f := range result.Files {
		fmt.Printf("    %s (%d lines)\n", f.Path, f.Lines)
	}
	fmt.Printf("  Total: %d lines\n", result.TotalLines)
	fmt.Printf("  Next: Fill sections following TODO markers, then run `ddis checkpoint`\n")

	// ddis:maintains APP-INV-053 (event stream completeness — emits artifact_written to stream 1)
	emitEvent(".", events.StreamDiscovery, events.TypeArtifactWritten, "", map[string]interface{}{
		"output_dir":  result.OutputDir,
		"total_lines": result.TotalLines,
		"file_count":  len(result.Files),
		"command":     "skeleton",
	})

	return nil
}
