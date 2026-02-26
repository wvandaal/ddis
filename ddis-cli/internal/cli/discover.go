package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/autoprompt"
	"github.com/wvandaal/ddis/internal/discover"
	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:implements APP-ADR-020 (conversational over procedural)
// ddis:maintains APP-INV-027 (thread topology primacy)
// ddis:maintains APP-INV-029 (convergent thread selection)

var (
	discoverSpec    string
	discoverThread  string
	discoverContent string
	discoverDepth   int
	discoverEvents  string
)

var discoverCmd = &cobra.Command{
	Use:   "discover",
	Short: "Conversational specification discovery",
	Long: `Opens a discovery context for conversational specification authoring.
The CLI generates context and guidance for an external LLM interpreter.

Threads are the primary organizational unit. The system infers the active
thread from content (convergent selection), or you can override with --thread.

Subcommands:
  status    Show discovery state summary
  threads   List all inquiry threads
  park      Park a thread for later resumption
  merge     Merge one thread into another

Examples:
  ddis discover --spec index.db
  ddis discover --spec index.db --thread t-cache-design
  ddis discover --spec index.db --content "how should we handle TTL?"
  ddis discover status --spec index.db
  ddis discover threads --spec index.db`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runDiscover,
	SilenceErrors: true,
	SilenceUsage:  true,
}

var discoverStatusCmd = &cobra.Command{
	Use:   "status",
	Short: "Show discovery state summary",
	RunE:  runDiscoverStatus,
	SilenceErrors: true,
	SilenceUsage:  true,
}

var discoverThreadsCmd = &cobra.Command{
	Use:   "threads",
	Short: "List all inquiry threads",
	RunE:  runDiscoverThreads,
	SilenceErrors: true,
	SilenceUsage:  true,
}

var discoverParkCmd = &cobra.Command{
	Use:   "park",
	Short: "Park a thread for later resumption",
	RunE:  runDiscoverPark,
	SilenceErrors: true,
	SilenceUsage:  true,
}

var discoverMergeCmd = &cobra.Command{
	Use:   "merge <source-thread>",
	Short: "Merge one thread into another",
	Args:  cobra.ExactArgs(1),
	RunE:  runDiscoverMerge,
	SilenceErrors: true,
	SilenceUsage:  true,
}

var discoverMergeInto string

func init() {
	discoverCmd.PersistentFlags().StringVar(&discoverSpec, "spec", "", "Path to spec database")
	discoverCmd.PersistentFlags().StringVar(&discoverEvents, "events", ".ddis/events", "Path to events directory")

	discoverCmd.PersistentFlags().StringVar(&discoverThread, "thread", "", "Explicit thread override")
	discoverCmd.Flags().StringVar(&discoverContent, "content", "", "Content for thread matching")
	discoverCmd.Flags().IntVar(&discoverDepth, "depth", 0, "Conversation depth for k* budget")

	discoverMergeCmd.Flags().StringVar(&discoverMergeInto, "into", "", "Target thread for merge")

	discoverCmd.AddCommand(discoverStatusCmd)
	discoverCmd.AddCommand(discoverThreadsCmd)
	discoverCmd.AddCommand(discoverParkCmd)
	discoverCmd.AddCommand(discoverMergeCmd)
	discoverCmd.AddCommand(crystallizeCmd)
}

func runDiscover(cmd *cobra.Command, args []string) error {
	// Accept positional DB arg alongside --spec flag
	specDB := discoverSpec
	if specDB == "" && len(args) >= 1 {
		specDB = args[0]
	}

	opts := discover.DiscoverOptions{
		SpecDB:    specDB,
		ThreadID:  discoverThread,
		Content:   discoverContent,
		Depth:     discoverDepth,
		EventsDir: discoverEvents,
	}

	var result *autoprompt.CommandResult
	var err error
	var db storage.DB
	var specID int64

	if opts.SpecDB != "" {
		var dbErr error
		db, dbErr = storage.Open(opts.SpecDB)
		if dbErr != nil {
			return fmt.Errorf("open spec database: %w", dbErr)
		}
		defer db.Close()

		var sErr error
		specID, sErr = storage.GetFirstSpecID(db)
		if sErr != nil {
			return fmt.Errorf("no spec found: %w", sErr)
		}
		result, err = discover.BuildContext(db, specID, opts)
	} else {
		result, err = discover.BuildContext(nil, 0, opts)
	}
	if err != nil {
		return fmt.Errorf("discover: %w", err)
	}

	out, err := result.RenderJSON()
	if err != nil {
		return err
	}
	fmt.Println(out)

	// Emit mode_observed event to Stream 1 (Discovery).
	specHash := ""
	dbPathForEvent := "."
	if opts.SpecDB != "" {
		dbPathForEvent = opts.SpecDB
		specHash = specHashFromDB(db, specID)
	}
	emitEvent(dbPathForEvent, events.StreamDiscovery, events.TypeModeObserved, specHash, map[string]interface{}{
		"thread_id": opts.ThreadID,
		"content":   opts.Content,
		"depth":     opts.Depth,
	})

	return nil
}

func runDiscoverStatus(cmd *cobra.Command, args []string) error {
	if discoverSpec == "" {
		result, err := discover.Status(nil, 0, discoverEvents)
		if err != nil {
			return fmt.Errorf("discover status: %w", err)
		}
		out, err := result.RenderJSON()
		if err != nil {
			return err
		}
		fmt.Println(out)
		return nil
	}

	db, err := storage.Open(discoverSpec)
	if err != nil {
		return fmt.Errorf("open spec database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	result, err := discover.Status(db, specID, discoverEvents)
	if err != nil {
		return fmt.Errorf("discover status: %w", err)
	}
	out, err := result.RenderJSON()
	if err != nil {
		return err
	}
	fmt.Println(out)
	return nil
}

func runDiscoverThreads(cmd *cobra.Command, args []string) error {
	result, err := discover.ListThreads(discoverEvents)
	if err != nil {
		return fmt.Errorf("discover threads: %w", err)
	}
	out, err := result.RenderJSON()
	if err != nil {
		return err
	}
	fmt.Println(out)
	return nil
}

func runDiscoverPark(cmd *cobra.Command, args []string) error {
	threadID := discoverThread
	if threadID == "" {
		return fmt.Errorf("--thread is required for park")
	}
	if err := discover.ParkThread(discoverEvents, threadID); err != nil {
		return fmt.Errorf("park thread: %w", err)
	}
	fmt.Printf("Thread %s parked.\n", threadID)

	// Emit thread_parked event to Stream 1 (Discovery).
	emitEvent(".", events.StreamDiscovery, events.TypeThreadParked, "", map[string]interface{}{
		"thread_id": threadID,
	})

	return nil
}

func runDiscoverMerge(cmd *cobra.Command, args []string) error {
	sourceID := args[0]
	if discoverMergeInto == "" {
		return fmt.Errorf("--into is required for merge")
	}
	if err := discover.MergeThread(discoverEvents, sourceID, discoverMergeInto); err != nil {
		return fmt.Errorf("merge thread: %w", err)
	}
	fmt.Printf("Thread %s merged into %s.\n", sourceID, discoverMergeInto)

	// Emit thread_merged event to Stream 1 (Discovery).
	emitEvent(".", events.StreamDiscovery, events.TypeThreadMerged, "", map[string]interface{}{
		"source_thread": sourceID,
		"target_thread": discoverMergeInto,
	})

	return nil
}
