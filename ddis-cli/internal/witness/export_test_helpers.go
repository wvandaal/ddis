package witness

// Export internal functions for external behavioral tests.
// These wrappers exist solely so tests/invariant_behavioral_test.go
// can exercise the majority-vote logic without duplicating code.

// ClassifyResponseForTest exposes classifyResponse for testing.
func ClassifyResponseForTest(resp string) string {
	return classifyResponse(resp)
}

// MajorityVoteForTest exposes majorityVote for testing.
func MajorityVoteForTest(votes map[string]int) (int, string) {
	return majorityVote(votes)
}

// RequiredRunsForTest exposes the requiredRuns constant for testing.
func RequiredRunsForTest() int {
	return requiredRuns
}
