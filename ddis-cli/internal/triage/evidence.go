package triage

// ddis:maintains APP-INV-065 (resolution evidence chain — per-invariant witness + challenge verification)
// ddis:implements APP-ADR-055 (full agent autonomy with guardrails — evidence-chain gate)

import (
	"database/sql"
	"fmt"

	"github.com/wvandaal/ddis/internal/events"
	"github.com/wvandaal/ddis/internal/storage"
)

// VerifyEvidenceChain checks that every affected invariant has a non-stale witness
// with a confirmed challenge verdict. Returns the complete chain if all evidence
// is present, or a list of violations describing what's missing.
func VerifyEvidenceChain(db *sql.DB, specID int64, issueNumber int, evts []events.Event) (*EvidenceChain, []Violation) {
	affected := ExtractAffectedInvariants(evts, issueNumber)
	if len(affected) == 0 {
		return nil, []Violation{{
			Type:   "no_affected_invariants",
			Detail: fmt.Sprintf("issue %d has no affected invariants in event stream", issueNumber),
			Remedy: "ddis issue triage with --affected flag to declare affected invariants",
		}}
	}

	var violations []Violation
	var entries []EvidenceEntry

	// Load witnesses and challenges once (O(n) total)
	witnesses, wErr := storage.ListWitnesses(db, specID)
	if wErr != nil {
		witnesses = nil
	}
	challenges, _ := storage.ListChallengeResults(db, specID)

	for _, invID := range affected {
		// Find best valid witness for this invariant
		var bestWitness *storage.InvariantWitness
		for i := range witnesses {
			if witnesses[i].InvariantID == invID && witnesses[i].Status == "valid" {
				bestWitness = &witnesses[i]
				break
			}
		}

		if bestWitness == nil {
			violations = append(violations, Violation{
				InvariantID: invID,
				Type:        "missing_witness",
				Detail:      fmt.Sprintf("no valid witness for %s", invID),
				Remedy:      fmt.Sprintf("ddis witness %s --type test --by <agent>", invID),
			})
			continue
		}

		// Check for staleness: witness spec_hash vs current spec hash
		inv, err := storage.GetInvariant(db, specID, invID)
		if err == nil && inv != nil && bestWitness.SpecHash != inv.ContentHash {
			violations = append(violations, Violation{
				InvariantID: invID,
				Type:        "stale_witness",
				Detail:      fmt.Sprintf("witness spec_hash %s != current %s", bestWitness.SpecHash, inv.ContentHash),
				Remedy:      fmt.Sprintf("ddis witness %s --type test --by <agent> (re-witness after spec change)", invID),
			})
			continue
		}

		// Find challenge result for this invariant
		var bestChallenge *storage.ChallengeResult
		for i := range challenges {
			if challenges[i].InvariantID == invID {
				bestChallenge = &challenges[i]
				break
			}
		}

		if bestChallenge == nil {
			violations = append(violations, Violation{
				InvariantID: invID,
				Type:        "missing_challenge",
				Detail:      fmt.Sprintf("no challenge result for %s", invID),
				Remedy:      fmt.Sprintf("ddis challenge %s --code-root .", invID),
			})
			continue
		}

		if bestChallenge.Verdict != "confirmed" {
			violations = append(violations, Violation{
				InvariantID: invID,
				Type:        "non_confirmed",
				Detail:      fmt.Sprintf("challenge verdict is %q, not confirmed", bestChallenge.Verdict),
				Remedy:      fmt.Sprintf("fix implementation for %s, re-witness, then ddis challenge %s", invID, invID),
			})
			continue
		}

		entries = append(entries, EvidenceEntry{
			InvariantID: invID,
			WitnessID:   bestWitness.ID,
			WitnessType: bestWitness.EvidenceType,
			ChallengeID: bestChallenge.ID,
			Verdict:     bestChallenge.Verdict,
		})
	}

	if len(violations) > 0 {
		return nil, violations
	}

	return &EvidenceChain{
		IssueNumber: issueNumber,
		Entries:     entries,
		Complete:    true,
	}, nil
}
