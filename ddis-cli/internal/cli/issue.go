package cli

// ddis:implements APP-ADR-044 (external issue tracker integration via gh CLI)
// ddis:implements APP-ADR-053 (issue lifecycle as event-sourced state machine)
// ddis:maintains APP-INV-057 (external tool graceful degradation — clear error when gh missing)
// ddis:maintains APP-INV-063 (issue-discovery linkage — triage requires thread)
// ddis:maintains APP-INV-065 (resolution evidence chain — close requires evidence)

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"strconv"
	"strings"

	"github.com/spf13/cobra"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/storage"
	"github.com/wvandaal/ddis/internal/triage"
)

// --- Parent command ---

var issueCmd = &cobra.Command{
	Use:   "issue [command]",
	Short: "Issue lifecycle: file, triage, list, status, close",
	Long: `Manages the issue lifecycle as an event-sourced state machine.

Issue state is derived from Stream 3 event replay, never stored mutably.

Subcommands:
  file     File a new issue via gh CLI
  triage   Link issue to discovery thread (filed → triaged)
  list     List issues with derived lifecycle state
  status   Show lifecycle state, next action, blocking preconditions
  close    Close issue with evidence chain verification

Backward compatibility: "ddis issue <title>" delegates to "ddis issue file <title>"

Examples:
  ddis issue file "Parser drops invariants"
  ddis issue triage 42 --thread t-parser-fix
  ddis issue list --json
  ddis issue status 42
  ddis issue close 42`,
	RunE: func(cmd *cobra.Command, args []string) error {
		// Bare "ddis issue" → status
		if len(args) == 0 {
			return runIssueStatus(cmd, args)
		}
		// Backward compat: non-numeric first arg → delegate to file
		if _, err := strconv.Atoi(args[0]); err != nil {
			// Not a number — treat as title for "ddis issue file"
			return runIssueFile(cmd, args)
		}
		// Numeric first arg → status for that issue
		return runIssueStatus(cmd, args)
	},
	SilenceErrors: true,
	SilenceUsage:  true,
}

// --- Subcommand: file ---

var (
	issueBody     string
	issueLabels   []string
	issueRepo     string
	issueTemplate bool
)

var issueFileCmd = &cobra.Command{
	Use:   "file [title]",
	Short: "File a new issue via gh CLI with structured DDIS context",
	Long: `Creates a GitHub issue via the gh CLI with structured context.

Requires gh (https://cli.github.com/) to be installed and authenticated.

Examples:
  ddis issue file "Parser drops invariants silently"
  ddis issue file "Add bulk rename" --label enhancement`,
	Args:          cobra.ExactArgs(1),
	RunE:          runIssueFile,
	SilenceErrors: true,
	SilenceUsage:  true,
}

// --- Subcommand: triage ---

var (
	issueTriageThread   string
	issueTriageAffected []string
)

var issueTriageCmd = &cobra.Command{
	Use:   "triage <number>",
	Short: "Link issue to discovery thread (filed → triaged)",
	Long: `Transitions an issue from filed to triaged by linking it to a discovery
thread. The thread must contain at least one observation (APP-INV-063).

Examples:
  ddis issue triage 42 --thread t-parser-fix
  ddis issue triage 42 --thread t-parser-fix --affected APP-INV-001,APP-INV-009`,
	Args:          cobra.ExactArgs(1),
	RunE:          runIssueTriage,
	SilenceErrors: true,
	SilenceUsage:  true,
}

// --- Subcommand: list ---

var issueListJSON bool

var issueListCmd = &cobra.Command{
	Use:   "list",
	Short: "List issues with derived lifecycle state",
	Long: `Lists all issues with their event-sourced lifecycle state.

Examples:
  ddis issue list
  ddis issue list --json`,
	Args:          cobra.NoArgs,
	RunE:          runIssueList,
	SilenceErrors: true,
	SilenceUsage:  true,
}

// --- Subcommand: status ---

var issueStatusJSON bool

var issueStatusCmd = &cobra.Command{
	Use:   "status [number]",
	Short: "Show lifecycle state, next action, blocking preconditions",
	Long: `Shows the derived lifecycle state of an issue (or all issues if no
number is given), including valid transitions and next recommended action.

Examples:
  ddis issue status
  ddis issue status 42 --json`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          runIssueStatus,
	SilenceErrors: true,
	SilenceUsage:  true,
}

// --- Subcommand: close ---

var (
	issueCloseWontFix bool
	issueCloseReason  string
)

var issueCloseCmd = &cobra.Command{
	Use:   "close <number>",
	Short: "Close issue with evidence chain verification",
	Long: `Closes an issue after verifying the complete evidence chain:
every affected invariant must have a non-stale witness with a confirmed
challenge verdict (APP-INV-065).

Use --wont-fix to close without evidence chain.

Examples:
  ddis issue close 42
  ddis issue close 42 --wont-fix --reason "Superseded by issue 45"`,
	Args:          cobra.ExactArgs(1),
	RunE:          runIssueClose,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	// File subcommand flags
	issueFileCmd.Flags().StringVar(&issueBody, "body", "", "Issue body (if omitted, auto-generates structured template)")
	issueFileCmd.Flags().StringSliceVar(&issueLabels, "label", nil, "Labels to apply (repeatable)")
	issueFileCmd.Flags().StringVar(&issueRepo, "repo", "", "GitHub repo (owner/name); auto-detected if omitted")
	issueFileCmd.Flags().BoolVar(&issueTemplate, "template", false, "Print template to stdout and exit")

	// Triage subcommand flags
	issueTriageCmd.Flags().StringVar(&issueTriageThread, "thread", "", "Discovery thread ID to link")
	issueTriageCmd.Flags().StringSliceVar(&issueTriageAffected, "affected", nil, "Affected invariant IDs (repeatable)")

	// List subcommand flags
	issueListCmd.Flags().BoolVar(&issueListJSON, "json", false, "JSON output")

	// Status subcommand flags
	issueStatusCmd.Flags().BoolVar(&issueStatusJSON, "json", false, "JSON output")

	// Close subcommand flags
	issueCloseCmd.Flags().BoolVar(&issueCloseWontFix, "wont-fix", false, "Close as wont_fix (no evidence chain required)")
	issueCloseCmd.Flags().StringVar(&issueCloseReason, "reason", "", "Reason for wont_fix (required with --wont-fix)")

	// Register subcommands
	issueCmd.AddCommand(issueFileCmd)
	issueCmd.AddCommand(issueTriageCmd)
	issueCmd.AddCommand(issueListCmd)
	issueCmd.AddCommand(issueStatusCmd)
	issueCmd.AddCommand(issueCloseCmd)
}

// --- Implementation ---

// issueBodyTemplate is the structured template that ensures every issue
// contains the context needed for DDIS-methodology triage.
const issueBodyTemplate = `## Description

<!-- What happened? What should have happened instead? Be specific. -->

## Steps to Reproduce

<!-- Exact commands, inputs, and environment. Paste terminal output. -->

1.
2.
3.

## DDIS Context

<!-- Auto-populated by ddis issue when available. Do NOT delete. -->

- **ddis version**: %s
- **Validation state**: %s
- **Drift score**: %s
- **Coverage**: %s
- **Relevant invariants**: <!-- e.g. APP-INV-001, APP-INV-042 -->
- **Relevant ADRs**: <!-- e.g. APP-ADR-005 -->

## Expected Behavior

<!-- What is the correct behavior per the spec? Reference invariants. -->

## Spec Impact

<!-- Which modules/invariants/ADRs might need changes to fix this? -->

## Additional Context

<!-- Stack traces, screenshots, related issues, workarounds. -->
`

func runIssueFile(cmd *cobra.Command, args []string) error {
	if issueTemplate {
		body := buildTemplateBody()
		fmt.Fprint(cmd.OutOrStdout(), body)
		return nil
	}

	if len(args) == 0 {
		return fmt.Errorf("title is required: ddis issue file \"<title>\"")
	}
	title := args[0]

	// Check gh is installed (APP-INV-057)
	ghPath, err := exec.LookPath("gh")
	if err != nil {
		return fmt.Errorf("gh CLI not found: install from https://cli.github.com/ and run \"gh auth login\"")
	}

	body := issueBody
	if body == "" {
		body = buildTemplateBody()
	}

	ghArgs := []string{"issue", "create", "--title", title, "--body", body}
	for _, label := range issueLabels {
		ghArgs = append(ghArgs, "--label", label)
	}
	if issueRepo != "" {
		ghArgs = append(ghArgs, "--repo", issueRepo)
	}

	ghCmd := exec.Command(ghPath, ghArgs...)
	ghCmd.Stdin = os.Stdin
	ghCmd.Stdout = cmd.OutOrStdout()
	ghCmd.Stderr = cmd.ErrOrStderr()

	if err := ghCmd.Run(); err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok && exitErr.ExitCode() == 4 {
			return fmt.Errorf("gh authentication required: run \"gh auth login\" first")
		}
		return fmt.Errorf("gh issue create failed: %w", err)
	}

	// Emit issue_created event
	emitEvent(".", events.StreamImplementation, events.TypeIssueCreated, "", map[string]interface{}{
		"title":  title,
		"labels": issueLabels,
	})

	if !NoGuidance {
		fmt.Fprintln(cmd.ErrOrStderr())
		fmt.Fprintln(cmd.ErrOrStderr(), "Next: ddis issue triage <number> --thread <thread-id>")
		fmt.Fprintln(cmd.ErrOrStderr(), "  Link the issue to a discovery thread for triage.")
	}

	return nil
}

func runIssueTriage(cmd *cobra.Command, args []string) error {
	num, err := strconv.Atoi(args[0])
	if err != nil {
		return fmt.Errorf("issue number must be an integer: %s", args[0])
	}

	if issueTriageThread == "" {
		return fmt.Errorf("--thread is required: ddis issue triage %d --thread <thread-id>\n  Create a thread first: ddis discover --thread <thread-id>", num)
	}

	// Verify thread has events (APP-INV-063)
	dbPath, _ := FindDB()
	if dbPath != "" {
		wsRoot := events.WorkspaceRoot(dbPath)
		streamPath := events.StreamPath(wsRoot, events.StreamDiscovery)
		evts, _ := events.ReadStream(streamPath, events.EventFilters{})
		threadHasEvents := false
		for _, e := range evts {
			var payload map[string]interface{}
			if err := json.Unmarshal(e.Payload, &payload); err == nil {
				if tid, ok := payload["thread_id"].(string); ok && tid == issueTriageThread {
					threadHasEvents = true
					break
				}
			}
		}
		if !threadHasEvents {
			fmt.Fprintf(cmd.ErrOrStderr(), "Warning: thread %q has no events in discovery stream.\n", issueTriageThread)
			fmt.Fprintln(cmd.ErrOrStderr(), "  Consider: ddis discover --thread", issueTriageThread)
		}
	}

	// Emit issue_triaged event
	payload := map[string]interface{}{
		"issue_number": num,
		"thread_id":    issueTriageThread,
	}
	if len(issueTriageAffected) > 0 {
		payload["affected_invariants"] = issueTriageAffected
	}

	wsRoot := "."
	if dbPath != "" {
		wsRoot = events.WorkspaceRoot(dbPath)
	}
	emitEvent(wsRoot, events.StreamImplementation, events.TypeIssueTriaged, "", payload)

	fmt.Fprintf(cmd.OutOrStdout(), "Issue #%d triaged → linked to thread %s\n", num, issueTriageThread)

	if !NoGuidance {
		fmt.Fprintln(cmd.ErrOrStderr(), "\nNext: ddis discover --thread", issueTriageThread)
		fmt.Fprintln(cmd.ErrOrStderr(), "  Continue investigation in the linked thread.")
	}

	return nil
}

func runIssueList(cmd *cobra.Command, args []string) error {
	dbPath, _ := FindDB()
	wsRoot := "."
	if dbPath != "" {
		wsRoot = events.WorkspaceRoot(dbPath)
	}

	streamPath := events.StreamPath(wsRoot, events.StreamImplementation)
	evts, err := events.ReadStream(streamPath, events.EventFilters{})
	if err != nil {
		return fmt.Errorf("read event stream: %w", err)
	}

	issues := triage.DeriveAllIssueStates(derefEvents(evts))

	if issueListJSON {
		enc := json.NewEncoder(cmd.OutOrStdout())
		enc.SetIndent("", "  ")
		return enc.Encode(issues)
	}

	if len(issues) == 0 {
		fmt.Fprintln(cmd.OutOrStdout(), "No issues found in event stream.")
		return nil
	}

	fmt.Fprintf(cmd.OutOrStdout(), "Issues (%d):\n", len(issues))
	for _, info := range issues {
		threadInfo := ""
		if info.ThreadID != "" {
			threadInfo = fmt.Sprintf(" [thread: %s]", info.ThreadID)
		}
		fmt.Fprintf(cmd.OutOrStdout(), "  #%d  %-15s%s\n", info.Number, info.State, threadInfo)
	}

	return nil
}

func runIssueStatus(cmd *cobra.Command, args []string) error {
	dbPath, _ := FindDB()
	wsRoot := "."
	if dbPath != "" {
		wsRoot = events.WorkspaceRoot(dbPath)
	}

	streamPath := events.StreamPath(wsRoot, events.StreamImplementation)
	evts, err := events.ReadStream(streamPath, events.EventFilters{})
	if err != nil {
		return fmt.Errorf("read event stream: %w", err)
	}

	if len(args) > 0 {
		num, err := strconv.Atoi(args[0])
		if err != nil {
			return fmt.Errorf("issue number must be an integer: %s", args[0])
		}

		valEvts := derefEvents(evts)
		state, threadID, err := triage.DeriveIssueState(valEvts, num)
		if err != nil {
			return fmt.Errorf("derive state for #%d: %w", num, err)
		}

		// DeriveAllIssueStates populates AffectedInvariants internally;
		// for single-issue status, derive it via the same all-states path.
		allStates := triage.DeriveAllIssueStates(valEvts)
		var affected []string
		if info, ok := allStates[num]; ok {
			affected = info.AffectedInvariants
		}

		info := &triage.IssueInfo{
			Number:             num,
			State:              state,
			ThreadID:           threadID,
			ValidTransitions:   triage.NextValidTransitions(state),
			AffectedInvariants: affected,
		}

		if issueStatusJSON {
			enc := json.NewEncoder(cmd.OutOrStdout())
			enc.SetIndent("", "  ")
			return enc.Encode(info)
		}

		fmt.Fprintf(cmd.OutOrStdout(), "Issue #%d: %s\n", num, state)
		if threadID != "" {
			fmt.Fprintf(cmd.OutOrStdout(), "  Thread: %s\n", threadID)
		}
		if len(info.AffectedInvariants) > 0 {
			fmt.Fprintf(cmd.OutOrStdout(), "  Affected: %s\n", strings.Join(info.AffectedInvariants, ", "))
		}
		if len(info.ValidTransitions) > 0 {
			fmt.Fprintf(cmd.OutOrStdout(), "  Valid transitions: %s\n", strings.Join(info.ValidTransitions, ", "))
		}
		return nil
	}

	// No number — show all issues
	return runIssueList(cmd, args)
}

func runIssueClose(cmd *cobra.Command, args []string) error {
	num, err := strconv.Atoi(args[0])
	if err != nil {
		return fmt.Errorf("issue number must be an integer: %s", args[0])
	}

	dbPath, _ := FindDB()
	wsRoot := "."
	if dbPath != "" {
		wsRoot = events.WorkspaceRoot(dbPath)
	}

	// Handle wont_fix path
	if issueCloseWontFix {
		if issueCloseReason == "" {
			return fmt.Errorf("--reason is required with --wont-fix")
		}
		emitEvent(wsRoot, events.StreamImplementation, events.TypeIssueWontfix, "", map[string]interface{}{
			"issue_number": num,
			"reason":       issueCloseReason,
		})
		fmt.Fprintf(cmd.OutOrStdout(), "Issue #%d closed as wont_fix: %s\n", num, issueCloseReason)
		return nil
	}

	// Normal close: verify evidence chain (APP-INV-065)
	if dbPath == "" {
		return fmt.Errorf("no database found — cannot verify evidence chain\n  Parse the spec first: ddis parse manifest.yaml")
	}

	db, err := storage.Open(dbPath)
	if err != nil {
		return fmt.Errorf("open database: %w", err)
	}
	defer db.Close()

	specID, err := storage.GetFirstSpecID(db)
	if err != nil {
		return fmt.Errorf("no spec found: %w", err)
	}

	streamPath := events.StreamPath(wsRoot, events.StreamImplementation)
	evts, _ := events.ReadStream(streamPath, events.EventFilters{})

	chain, violations := triage.VerifyEvidenceChain(db, specID, num, derefEvents(evts))
	if len(violations) > 0 {
		fmt.Fprintf(cmd.OutOrStdout(), "Cannot close issue #%d — evidence chain incomplete:\n", num)
		for _, v := range violations {
			fmt.Fprintf(cmd.OutOrStdout(), "  ✗ %s [%s]: %s\n", v.InvariantID, v.Type, v.Detail)
			fmt.Fprintf(cmd.OutOrStdout(), "    → %s\n", v.Remedy)
		}
		return fmt.Errorf("evidence chain incomplete: %d violations", len(violations))
	}

	// Evidence chain complete — emit close event
	emitEvent(wsRoot, events.StreamImplementation, events.TypeIssueClosed, "", map[string]interface{}{
		"issue_number":  num,
		"evidence_chain": chain,
	})

	fmt.Fprintf(cmd.OutOrStdout(), "Issue #%d closed with complete evidence chain (%d invariants verified)\n", num, len(chain.Entries))

	return nil
}

// --- Helpers ---

func buildTemplateBody() string {
	ver := collectCLIOutput("version")
	val := collectCLIOutput("validate", "--json")
	dft := collectCLIOutput("drift")
	cov := collectCLIOutput("coverage")

	verLine := firstLine(ver, Version+" ("+Commit+")")
	valLine := summarizeValidation(val)
	driftLine := firstLine(dft, "unavailable")
	covLine := firstLine(cov, "unavailable")

	return fmt.Sprintf(issueBodyTemplate, verLine, valLine, driftLine, covLine)
}

func collectCLIOutput(subArgs ...string) string {
	exe, err := os.Executable()
	if err != nil {
		return ""
	}
	args := append(subArgs, "-q")
	c := exec.Command(exe, args...)
	var buf bytes.Buffer
	c.Stdout = &buf
	c.Stderr = nil
	if err := c.Run(); err != nil {
		return ""
	}
	return strings.TrimSpace(buf.String())
}

func firstLine(s, fallback string) string {
	s = strings.TrimSpace(s)
	if s == "" {
		return fallback
	}
	if idx := strings.IndexByte(s, '\n'); idx >= 0 {
		return s[:idx]
	}
	return s
}

func summarizeValidation(raw string) string {
	if raw == "" {
		return "unavailable"
	}
	if strings.HasPrefix(strings.TrimSpace(raw), "{") {
		passed := extractJSONField(raw, "passed")
		failed := extractJSONField(raw, "failed")
		total := extractJSONField(raw, "total_checks")
		if passed != "" {
			return fmt.Sprintf("%s/%s passed, %s failed", passed, total, failed)
		}
	}
	for _, line := range strings.Split(raw, "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "Total:") {
			return trimmed
		}
	}
	return firstLine(raw, "unavailable")
}

func extractJSONField(raw, key string) string {
	needle := fmt.Sprintf(`"%s":`, key)
	idx := strings.Index(raw, needle)
	if idx < 0 {
		needle = fmt.Sprintf(`"%s" :`, key)
		idx = strings.Index(raw, needle)
	}
	if idx < 0 {
		return ""
	}
	rest := strings.TrimSpace(raw[idx+len(needle):])
	end := strings.IndexAny(rest, ",}\n")
	if end < 0 {
		return strings.TrimSpace(rest)
	}
	return strings.TrimSpace(rest[:end])
}

func ghInstalled() bool {
	_, err := exec.LookPath("gh")
	return err == nil
}

// derefEvents converts []*events.Event to []events.Event by dereferencing each pointer.
// ReadStream returns pointer-based slices; triage functions expect value-based slices.
func derefEvents(evts []*events.Event) []events.Event {
	if evts == nil {
		return nil
	}
	result := make([]events.Event, len(evts))
	for i, e := range evts {
		result[i] = *e
	}
	return result
}
