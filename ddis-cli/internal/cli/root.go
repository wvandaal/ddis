package cli

import (
	"errors"
	"fmt"
	"os"

	"github.com/spf13/cobra"
)

var rootCmd = &cobra.Command{
	Use:   "ddis",
	Short: "DDIS: Transactional Specification Management System",
	Long:  `Parses DDIS specifications into a structured SQLite index and renders them back to markdown.`,
}

// Execute runs the root command.
func Execute() {
	if err := rootCmd.Execute(); err != nil {
		if errors.Is(err, ErrValidationFailed) {
			os.Exit(1)
		}
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func init() {
	rootCmd.AddCommand(parseCmd)
	rootCmd.AddCommand(renderCmd)
	rootCmd.AddCommand(queryCmd)
	rootCmd.AddCommand(validateCmd)
	rootCmd.AddCommand(diffCmd)
	rootCmd.AddCommand(impactCmd)
	rootCmd.AddCommand(logCmd)
	rootCmd.AddCommand(txCmd)
	rootCmd.AddCommand(seedCmd)
	rootCmd.AddCommand(searchCmd)
	rootCmd.AddCommand(contextCmd)
	rootCmd.AddCommand(exemplarCmd)
	rootCmd.AddCommand(coverageCmd)
	rootCmd.AddCommand(stateCmd)
	rootCmd.AddCommand(skeletonCmd)
	rootCmd.AddCommand(checkpointCmd)
	rootCmd.AddCommand(checklistCmd)
	rootCmd.AddCommand(cascadeCmd)
	rootCmd.AddCommand(bundleCmd)
	rootCmd.AddCommand(implorderCmd)
	rootCmd.AddCommand(progressCmd)
	rootCmd.AddCommand(driftCmd)
}
