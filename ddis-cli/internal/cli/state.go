package cli

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/state"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:interfaces APP-INV-006 (transaction state machine)

var (
	stateSet    []string
	stateGet    string
	stateDelete string
	stateList   bool
	stateJSON   bool
)

var stateCmd = &cobra.Command{
	Use:   "state [db-path]",
	Short: "Manage session state key-value pairs",
	Long: `Read and write session state for a DDIS spec database.
State is stored as key-value pairs scoped to a spec.

Examples:
  ddis state index.db --set author=claude --set phase=7
  ddis state index.db --list
  ddis state index.db --get author
  ddis state index.db --delete author
  ddis state index.db --list --json`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runState,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	stateCmd.Flags().StringSliceVar(&stateSet, "set", nil, "Set key=value pairs (repeatable)")
	stateCmd.Flags().StringVar(&stateGet, "get", "", "Get value for key")
	stateCmd.Flags().StringVar(&stateDelete, "delete", "", "Delete key")
	stateCmd.Flags().BoolVar(&stateList, "list", false, "List all state entries")
	stateCmd.Flags().BoolVar(&stateJSON, "json", false, "Output as JSON")
}

func runState(cmd *cobra.Command, args []string) error {
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

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	// Handle --set
	if len(stateSet) > 0 {
		for _, kv := range stateSet {
			parts := strings.SplitN(kv, "=", 2)
			if len(parts) != 2 {
				return fmt.Errorf("invalid --set format, use key=value: %q", kv)
			}
			if err := state.Set(db, specID, parts[0], parts[1]); err != nil {
				return err
			}
		}
		fmt.Printf("Set %d key(s)\n", len(stateSet))
		// ddis:maintains APP-INV-053 (event stream completeness — emits status_changed to stream 3)
		emitEvent(dbPath, events.StreamImplementation, events.TypeStatusChanged, specHashFromDB(db, specID), map[string]interface{}{
			"keys_set": len(stateSet),
			"command":  "state",
			"action":   "set",
		})
		return nil
	}

	// Handle --get
	if stateGet != "" {
		val, err := state.Get(db, specID, stateGet)
		if err != nil {
			return err
		}
		fmt.Println(val)
		return nil
	}

	// Handle --delete
	if stateDelete != "" {
		if err := state.Delete(db, specID, stateDelete); err != nil {
			return err
		}
		// ddis:maintains APP-INV-053 (event stream completeness — emits status_changed to stream 3)
		emitEvent(dbPath, events.StreamImplementation, events.TypeStatusChanged, specHashFromDB(db, specID), map[string]interface{}{
			"key":     stateDelete,
			"command": "state",
			"action":  "delete",
		})
		return nil
	}

	// Handle --list (default if no other action)
	entries, err := state.List(db, specID)
	if err != nil {
		return err
	}
	if stateJSON {
		data, err := json.MarshalIndent(entries, "", "  ")
		if err != nil {
			return fmt.Errorf("marshal state to JSON: %w", err)
		}
		fmt.Println(string(data))
		return nil
	}
	if len(entries) == 0 {
		fmt.Println("No state entries.")
		return nil
	}
	for _, e := range entries {
		fmt.Printf("%-20s = %-30s (%s)\n", e.Key, e.Value, e.UpdatedAt)
	}
	return nil
}
