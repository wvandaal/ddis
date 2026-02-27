package cli

// ddis:implements APP-ADR-044 (external issue tracker integration via gh CLI)
// ddis:maintains APP-INV-057 (external tool graceful degradation — clear error when gh missing)

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"strings"

	"github.com/spf13/cobra"
)

var (
	issueBody     string
	issueLabels   []string
	issueRepo     string
	issueTemplate bool
)

var issueCmd = &cobra.Command{
	Use:   "issue [title]",
	Short: "Submit a bug or feature request to the DDIS GitHub repo",
	Long: `Creates a GitHub issue via the gh CLI with structured context for DDIS triage.

When --body is omitted, auto-generates a structured issue template that
collects: ddis version, validation state, drift score, coverage, and
reproduction steps. This ensures every filed issue contains sufficient
context for DDIS-methodology triage (discover → spec → implement → witness).

Use --template to print the blank template to stdout without filing.

Requires gh (https://cli.github.com/) to be installed and authenticated.
If gh is not found or not authenticated, prints a clear recovery hint.

The repo is auto-detected from the git remote, or can be overridden
with --repo.

Examples:
  ddis issue "Parser drops invariants silently"
  ddis issue "Parser drops invariants silently" --label bug
  ddis issue "Add bulk rename" --label enhancement --body "Detailed description..."
  ddis issue --template`,
	Args:          cobra.RangeArgs(0, 1),
	RunE:          runIssue,
	SilenceErrors: true,
	SilenceUsage:  true,
}

func init() {
	issueCmd.Flags().StringVar(&issueBody, "body", "", "Issue body (if omitted, auto-generates structured template with DDIS context)")
	issueCmd.Flags().StringSliceVar(&issueLabels, "label", nil, "Labels to apply (repeatable)")
	issueCmd.Flags().StringVar(&issueRepo, "repo", "", "GitHub repo (owner/name); auto-detected from git remote if omitted")
	issueCmd.Flags().BoolVar(&issueTemplate, "template", false, "Print the issue template to stdout and exit (does not file)")
}

// issueBodyTemplate is the structured template that ensures every issue
// contains the context needed for DDIS-methodology triage. Each section
// maps to a phase of the bilateral lifecycle:
//
//   Description    → ddis discover (what is the problem/idea?)
//   Reproduction   → ddis validate (how do we verify it?)
//   DDIS Context   → ddis drift/coverage (where does it sit in the spec?)
//   Expected       → ddis refine (what should the behavior be?)
//   Spec Impact    → ddis impact/cascade (what else does this touch?)
const issueBodyTemplate = `## Description

<!-- What happened? What should have happened instead? Be specific. -->

## Steps to Reproduce

<!-- Exact commands, inputs, and environment. Paste terminal output. -->

1.
2.
3.

## DDIS Context

<!-- Auto-populated by ddis issue when available. Do NOT delete. -->
<!-- These fields let maintainers triage using DDIS methodology:        -->
<!--   discover → spec the fix → implement → witness → challenge       -->

- **ddis version**: %s
- **Validation state**: %s
- **Drift score**: %s
- **Coverage**: %s
- **Relevant invariants**: <!-- e.g. APP-INV-001, APP-INV-042 -->
- **Relevant ADRs**: <!-- e.g. APP-ADR-005 -->

## Expected Behavior

<!-- What is the correct behavior per the spec? Reference invariants. -->

## Spec Impact

<!-- Which modules/invariants/ADRs might need changes to fix this?     -->
<!-- Run: ddis impact <element> or ddis cascade <element> to check.    -->

## Additional Context

<!-- Stack traces, screenshots, related issues, workarounds. -->
`

func runIssue(cmd *cobra.Command, args []string) error {
	// --template mode: print template and exit.
	if issueTemplate {
		body := buildTemplateBody()
		fmt.Fprint(cmd.OutOrStdout(), body)
		return nil
	}

	if len(args) == 0 {
		return fmt.Errorf("title is required: ddis issue \"<title>\"")
	}
	title := args[0]

	// Check gh is installed (APP-INV-057: graceful degradation).
	ghPath, err := exec.LookPath("gh")
	if err != nil {
		return fmt.Errorf("gh CLI not found: install from https://cli.github.com/ and run \"gh auth login\"")
	}

	// Auto-generate structured body if not provided.
	body := issueBody
	if body == "" {
		body = buildTemplateBody()
	}

	// Build gh issue create arguments.
	ghArgs := []string{"issue", "create", "--title", title, "--body", body}

	for _, label := range issueLabels {
		ghArgs = append(ghArgs, "--label", label)
	}

	if issueRepo != "" {
		ghArgs = append(ghArgs, "--repo", issueRepo)
	}

	// Shell out to gh. Inherit stdin so gh can prompt interactively.
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

	// Guidance postscript.
	if !NoGuidance {
		fmt.Fprintln(cmd.ErrOrStderr())
		fmt.Fprintln(cmd.ErrOrStderr(), "Next: ddis discover --content \"<issue-url>\"")
		fmt.Fprintln(cmd.ErrOrStderr(), "  Open a discovery thread to spec the fix using DDIS methodology.")
	}

	return nil
}

// buildTemplateBody populates the issue template with live DDIS context.
// Collects version, validation summary, drift, and coverage by running
// the CLI's own commands. Each collection is best-effort — if any fails,
// the field shows "unavailable" rather than blocking the issue.
func buildTemplateBody() string {
	ver := collectCLIOutput("version")
	val := collectCLIOutput("validate", "--json")
	drift := collectCLIOutput("drift")
	cov := collectCLIOutput("coverage")

	// Extract single-line summaries from command output.
	verLine := firstLine(ver, Version+" ("+Commit+")")
	valLine := summarizeValidation(val)
	driftLine := firstLine(drift, "unavailable")
	covLine := firstLine(cov, "unavailable")

	return fmt.Sprintf(issueBodyTemplate, verLine, valLine, driftLine, covLine)
}

// collectCLIOutput runs a ddis subcommand and captures its stdout.
// Returns empty string on any error — never blocks the issue flow.
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

// firstLine returns the first non-empty line of s, or fallback if empty.
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

// summarizeValidation extracts a one-line summary from validation output.
func summarizeValidation(raw string) string {
	if raw == "" {
		return "unavailable"
	}
	// JSON output: look for "passed"/"failed" keys.
	if strings.HasPrefix(strings.TrimSpace(raw), "{") {
		passed := extractJSONField(raw, "passed")
		failed := extractJSONField(raw, "failed")
		total := extractJSONField(raw, "total_checks")
		if passed != "" {
			return fmt.Sprintf("%s/%s passed, %s failed", passed, total, failed)
		}
	}
	// Text output: look for the "Total:" line.
	for _, line := range strings.Split(raw, "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "Total:") {
			return trimmed
		}
	}
	return firstLine(raw, "unavailable")
}

// extractJSONField does a crude extraction of a top-level numeric JSON field.
// Avoids importing encoding/json for a best-effort one-liner.
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
	// Read until comma, brace, or newline.
	end := strings.IndexAny(rest, ",}\n")
	if end < 0 {
		return strings.TrimSpace(rest)
	}
	return strings.TrimSpace(rest[:end])
}

// ghInstalled reports whether the gh CLI is available on PATH.
func ghInstalled() bool {
	_, err := exec.LookPath("gh")
	return err == nil
}

// buildGHArgs constructs the argument list for gh issue create.
func buildGHArgs(title, body, repo string, labels []string) []string {
	args := []string{"issue", "create", "--title", title}
	if body != "" {
		args = append(args, "--body", body)
	}
	for _, l := range labels {
		args = append(args, "--label", l)
	}
	if repo != "" {
		args = append(args, "--repo", repo)
	}
	return args
}
