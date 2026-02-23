package cli

import (
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
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func init() {
	rootCmd.AddCommand(parseCmd)
	rootCmd.AddCommand(renderCmd)
}
