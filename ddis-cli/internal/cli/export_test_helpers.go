package cli

// EmitRecoveryHintForTest is an exported wrapper around emitRecoveryHint
// for use in behavioral tests. This is the only way to test the error
// recovery guidance mechanism from outside the cli package.
func EmitRecoveryHintForTest(err error) {
	emitRecoveryHint(err)
}
