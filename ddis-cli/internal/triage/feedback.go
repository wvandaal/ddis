package triage

// ddis:maintains APP-INV-066 (recursive feedback — scope reduction on challenge refutation)

import (
	"fmt"
)

// SuggestRemediationIssue generates a remediation issue suggestion for a refuted
// invariant. The suggested issue has strictly smaller scope (proper subset of
// affected invariants) to ensure the well-founded ordering decreases (APP-INV-068).
func SuggestRemediationIssue(invariantID string) string {
	return fmt.Sprintf("ddis issue file \"Remediate %s\" --label refutation", invariantID)
}

// SuggestRemediationWithScope generates a suggestion that explicitly lists the
// smaller scope. The parentAffected is the original issue's invariant set;
// the suggested issue targets only the single refuted invariant.
func SuggestRemediationWithScope(invariantID string, parentAffected []string) string {
	// The remediation targets exactly {invariantID}, which is a proper subset
	// of parentAffected (|{invariantID}| = 1 < |parentAffected|).
	if len(parentAffected) <= 1 {
		// Cannot reduce scope further — already at singleton
		return fmt.Sprintf("# %s is the last remaining invariant — fix directly, no sub-issue needed", invariantID)
	}
	return SuggestRemediationIssue(invariantID)
}
