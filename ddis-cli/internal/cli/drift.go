package cli

import (
	"fmt"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/drift"
	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/search"
	"github.com/wvandaal/ddis/internal/storage"
)

// ddis:implements APP-ADR-012 (annotations over code manifest)

var (
	driftJSON   bool
	driftReport bool
	driftIntent bool
)

var driftCmd = &cobra.Command{
	Use:   "drift [db-path]",
	Short: "Measure and remediate spec-implementation drift",
	Long: `Analyzes divergence between specification and implementation, producing
either a full drift report (--report) or a targeted remediation package
for the highest-priority drift item (default).

The drift report decomposes divergence into three quality dimensions:
  - Correctness: spec says X, implementation violates X
  - Depth: implementation does X, spec is silent on X
  - Coherence: spec is internally inconsistent (orphan cross-refs)

Examples:
  ddis drift index.db                    # Next remediation package
  ddis drift index.db --report           # Full drift summary
  ddis drift index.db --report --json    # Machine-readable report
  ddis drift index.db --intent           # Include intent drift`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runDrift,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	driftCmd.Flags().BoolVar(&driftJSON, "json", false, "Output as JSON")
	driftCmd.Flags().BoolVar(&driftReport, "report", false, "Show full drift report (default: show next remediation)")
	driftCmd.Flags().BoolVar(&driftIntent, "intent", false, "Include intent drift measurement")
}

func runDrift(cmd *cobra.Command, args []string) error {
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

	WarnIfStale(db, specID)

	if driftReport {
		// Full report mode
		opts := drift.Options{
			AsJSON: driftJSON,
			Report: true,
			Intent: driftIntent,
		}
		report, err := drift.Analyze(db, specID, opts)
		if err != nil {
			return err
		}
		out, err := drift.Render(report, driftJSON)
		if err != nil {
			return err
		}
		fmt.Print(out)

		// Emit drift_measured event to Stream 2 (Specification).
		emitEvent(dbPath, events.StreamSpecification, events.TypeDriftMeasured, specHashFromDB(db, specID), map[string]interface{}{
			"effective_drift": report.EffectiveDrift,
			"correctness":    report.QualityBreakdown.Correctness,
			"depth":          report.QualityBreakdown.Depth,
			"coherence":      report.QualityBreakdown.Coherence,
			"mode":           "report",
		})

		// Guidance postscript for report mode
		if !NoGuidance && !driftJSON {
			emitDriftGuidance(report)
		}
		return nil
	}

	// Default: remediation mode
	// Build LSI index for similarity (optional, non-fatal)
	var lsi *search.LSIIndex
	docs, err := search.ExtractDocuments(db, specID)
	if err == nil && len(docs) > 0 {
		k := 50
		if len(docs) < k {
			k = len(docs)
		}
		lsi, _ = search.BuildLSI(docs, k)
	}

	pkg, err := drift.Remediate(db, specID, lsi)
	if err != nil {
		return err
	}

	out, err := drift.RenderRemediation(pkg, driftJSON)
	if err != nil {
		return err
	}
	fmt.Print(out)

	// Emit drift_measured event for remediation mode to Stream 2.
	if pkg != nil {
		emitEvent(dbPath, events.StreamSpecification, events.TypeDriftMeasured, specHashFromDB(db, specID), map[string]interface{}{
			"mode":   "remediate",
			"target": pkg.Target,
		})
	}

	return nil
}

func emitDriftGuidance(report *drift.DriftReport) {
	if report.EffectiveDrift == 0 {
		fmt.Println("\nNext: ddis validate")
		fmt.Println("  Drift is 0 — verify structural integrity.")
		return
	}
	q := report.QualityBreakdown
	if q.Correctness > 0 && len(report.ImplDrift.Details) > 0 {
		element := report.ImplDrift.Details[0].Element
		fmt.Printf("\nNext: ddis context %s\n", element)
		fmt.Println("  Correctness drift — investigate the first drifted element.")
	} else if q.Coherence > 0 {
		fmt.Println("\nNext: ddis validate --checks 1")
		fmt.Println("  Coherence drift — repair cross-references.")
	} else {
		fmt.Println("\nNext: ddis drift")
		fmt.Println("  Depth drift — get remediation package for the top item.")
	}
}
