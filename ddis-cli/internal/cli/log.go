package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/oplog"
)

var (
	logJSON  bool
	logType  string
	logTx    string
	logSince string
	logLimit int
)

var logCmd = &cobra.Command{
	Use:   "log [oplog.jsonl]",
	Short: "Browse the operation log",
	Long: `Reads and displays records from a DDIS operation log (JSONL file).
Supports filtering by record type, transaction ID, time range, and count.

Examples:
  ddis log .ddis/oplog.jsonl
  ddis log .ddis/oplog.jsonl --type diff
  ddis log .ddis/oplog.jsonl --tx tx-abc123
  ddis log .ddis/oplog.jsonl --limit 10 --json`,
	Args:          cobra.ExactArgs(1),
	RunE:          runLog,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	logCmd.Flags().BoolVar(&logJSON, "json", false, "Output as JSON")
	logCmd.Flags().StringVar(&logType, "type", "", "Filter by record type (diff, validate, transaction)")
	logCmd.Flags().StringVar(&logTx, "tx", "", "Filter by transaction ID")
	logCmd.Flags().StringVar(&logSince, "since", "", "Filter records after this RFC3339 timestamp")
	logCmd.Flags().IntVar(&logLimit, "limit", 0, "Maximum number of records to display")
}

func runLog(cmd *cobra.Command, args []string) error {
	logPath := args[0]

	opts := oplog.FilterOpts{
		TxID:  logTx,
		Since: logSince,
		Limit: logLimit,
	}

	if logType != "" {
		opts.Types = []oplog.RecordType{oplog.RecordType(logType)}
	}

	records, err := oplog.ReadFiltered(logPath, opts)
	if err != nil {
		return fmt.Errorf("read oplog: %w", err)
	}

	if records == nil {
		records = []*oplog.Record{}
	}

	out, err := oplog.RenderLog(records, logJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)

	return nil
}
