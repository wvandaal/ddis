package cli

// ddis:implements APP-ADR-031 (navigational guidance as postscript — -q flag, grouped help)
// ddis:maintains APP-INV-045 (universal auto-discovery — FindDB used by all DB-reading commands)
// ddis:maintains APP-INV-046 (error recovery guidance — emitRecoveryHint on actionable errors)

import (
	"errors"
	"fmt"
	"os"
	"strings"

	"github.com/spf13/cobra"
)

// NoGuidance suppresses "Next:" guidance postscripts when true.
var NoGuidance bool

var rootCmd = &cobra.Command{
	Use:   "ddis",
	Short: "DDIS: Transactional Specification Management System",
	Long: `DDIS: Transactional Specification Management System

Parses DDIS specifications into a structured SQLite index, validates them,
measures coverage and drift, and guides iterative improvement.

Run "ddis next" or bare "ddis" to see current status and next recommended action.`,
	RunE: func(cmd *cobra.Command, args []string) error {
		// Bare "ddis" delegates to next logic
		return runNext(cmd, args)
	},
	SilenceErrors: true,
	SilenceUsage:  true,
}

// Execute runs the root command.
func Execute() {
	if err := rootCmd.Execute(); err != nil {
		if errors.Is(err, ErrValidationFailed) {
			os.Exit(1)
		}
		fmt.Fprintln(os.Stderr, err)
		if !NoGuidance {
			emitRecoveryHint(err)
		}
		os.Exit(1)
	}
}

// emitRecoveryHint writes a Tip: line to stderr for actionable errors.
// Actionable categories: no_db, stale_db, bad_args, missing_spec, empty_query.
func emitRecoveryHint(err error) {
	msg := err.Error()
	switch {
	case strings.Contains(msg, "no .ddis.db file found"):
		fmt.Fprintln(os.Stderr, "Tip: ddis parse manifest.yaml")
	case strings.Contains(msg, "open database"):
		fmt.Fprintln(os.Stderr, "Tip: ddis parse manifest.yaml")
	case strings.Contains(msg, "no spec found"):
		fmt.Fprintln(os.Stderr, "Tip: ddis parse manifest.yaml")
	case strings.Contains(msg, "empty query"):
		fmt.Fprintln(os.Stderr, "Tip: ddis search \"<your query>\"")
	case strings.Contains(msg, "multiple .ddis.db files"):
		fmt.Fprintln(os.Stderr, "Tip: specify the database path explicitly, e.g. ddis validate manifest.ddis.db")
	case strings.Contains(msg, "read manifest"):
		fmt.Fprintln(os.Stderr, "Tip: ensure manifest.yaml exists in the current directory")
	case strings.Contains(msg, "unknown command"):
		fmt.Fprintln(os.Stderr, "Tip: ddis next")
	}
}

func init() {
	// Global flags
	rootCmd.PersistentFlags().BoolVarP(&NoGuidance, "no-guidance", "q", false, "Suppress navigational guidance postscripts")

	// Command groups by workflow phase
	coreGroup := &cobra.Group{ID: "core", Title: "Core Workflow:"}
	investigateGroup := &cobra.Group{ID: "investigate", Title: "Investigation:"}
	improvementGroup := &cobra.Group{ID: "improvement", Title: "Improvement:"}
	planningGroup := &cobra.Group{ID: "planning", Title: "Planning:"}
	utilityGroup := &cobra.Group{ID: "utility", Title: "Utility:"}

	rootCmd.AddGroup(coreGroup, investigateGroup, improvementGroup, planningGroup, utilityGroup)

	// Core workflow
	nextCmd.GroupID = "core"
	parseCmd.GroupID = "core"
	validateCmd.GroupID = "core"
	coverageCmd.GroupID = "core"
	driftCmd.GroupID = "core"

	// Investigation
	contextCmd.GroupID = "investigate"
	searchCmd.GroupID = "investigate"
	queryCmd.GroupID = "investigate"
	exemplarCmd.GroupID = "investigate"
	impactCmd.GroupID = "investigate"
	cascadeCmd.GroupID = "investigate"
	contradictCmd.GroupID = "investigate"
	historyCmd.GroupID = "investigate"

	// Improvement
	refineCmd.GroupID = "improvement"
	discoverCmd.GroupID = "improvement"
	absorbCmd.GroupID = "improvement"
	witnessCmd.GroupID = "improvement"
	scanCmd.GroupID = "improvement"
	challengeCmd.GroupID = "improvement"

	// Planning
	progressCmd.GroupID = "planning"
	implorderCmd.GroupID = "planning"
	checklistCmd.GroupID = "planning"
	bundleCmd.GroupID = "planning"
	skeletonCmd.GroupID = "planning"
	diffCmd.GroupID = "planning"
	tasksCmd.GroupID = "planning"

	// Utility
	renderCmd.GroupID = "utility"
	seedCmd.GroupID = "utility"
	logCmd.GroupID = "utility"
	txCmd.GroupID = "utility"
	stateCmd.GroupID = "utility"
	checkpointCmd.GroupID = "utility"
	initCmd.GroupID = "utility"
	patchCmd.GroupID = "utility"
	manifestCmd.GroupID = "utility"
	specCmd.GroupID = "core"
	agentHelpCmd.GroupID = "utility"

	rootCmd.AddCommand(nextCmd)
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
	rootCmd.AddCommand(scanCmd)
	rootCmd.AddCommand(initCmd)
	rootCmd.AddCommand(tasksCmd)
	rootCmd.AddCommand(refineCmd)
	rootCmd.AddCommand(discoverCmd)
	rootCmd.AddCommand(absorbCmd)
	rootCmd.AddCommand(witnessCmd)
	rootCmd.AddCommand(challengeCmd)
	rootCmd.AddCommand(patchCmd)
	rootCmd.AddCommand(manifestCmd)
	rootCmd.AddCommand(contradictCmd)
	rootCmd.AddCommand(specCmd)
	rootCmd.AddCommand(historyCmd)
	rootCmd.AddCommand(agentHelpCmd)
	rootCmd.AddCommand(versionCmd)
}
