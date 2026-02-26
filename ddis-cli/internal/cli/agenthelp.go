package cli

// ddis:maintains APP-INV-042 (guidance emission — agent-facing command catalog)

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"github.com/spf13/pflag"
)

var agentHelpCmd = &cobra.Command{
	Use:   "commands",
	Short: "Print JSON command catalog for LLM agents",
	Long: `Outputs a machine-readable JSON catalog of all ddis commands,
their usage patterns, groups, and flags. Designed for LLM agent consumption.

Examples:
  ddis commands
  ddis commands | jq '.[].name'`,
	Args:          cobra.NoArgs,
	RunE:          runAgentHelp,
	SilenceErrors: true,
	SilenceUsage:  true,
}

type agentCommand struct {
	Name  string   `json:"name"`
	Use   string   `json:"use"`
	Short string   `json:"short"`
	Group string   `json:"group"`
	Flags []string `json:"flags,omitempty"`
}

func runAgentHelp(cmd *cobra.Command, args []string) error {
	var commands []agentCommand
	for _, c := range rootCmd.Commands() {
		if c.Hidden || c.Name() == "help" || c.Name() == "completion" {
			continue
		}
		ac := agentCommand{
			Name:  c.Name(),
			Use:   c.Use,
			Short: c.Short,
			Group: c.GroupID,
		}
		c.Flags().VisitAll(func(f *pflag.Flag) {
			ac.Flags = append(ac.Flags, fmt.Sprintf("--%s", f.Name))
		})
		commands = append(commands, ac)
	}

	enc := json.NewEncoder(os.Stdout)
	enc.SetIndent("", "  ")
	return enc.Encode(commands)
}
