package autoprompt

// ddis:maintains APP-INV-035 (guidance attenuation)
// ddis:maintains APP-INV-023 (prompt self-containment)

const (
	BaseBudget = 12  // ceiling from Gestalt Study 1
	Step       = 5   // invocations per budget decrement
	Floor      = 3   // minimum viable guidance
	MaxTokens  = 2000
	MinTokens  = 300
)

// KStarEff computes the effective attention budget for a given conversation depth.
// Returns a value between Floor (3) and BaseBudget (12).
func KStarEff(depth int) int {
	if depth < 0 {
		depth = 0
	}
	k := BaseBudget - (depth / Step)
	if k < Floor {
		return Floor
	}
	return k
}

// TokenTarget computes the maximum guidance tokens for a given conversation depth.
// Linear interpolation: k*=12 -> 2000, k*=3 -> 300.
func TokenTarget(depth int) int {
	k := KStarEff(depth)
	return MinTokens + (k-Floor)*(MaxTokens-MinTokens)/(BaseBudget-Floor)
}

// Attenuation computes how much to shrink guidance relative to first invocation.
// 0.0 = full guidance (depth 0), 0.75 = maximum attenuation (depth >= 45).
func Attenuation(depth int) float64 {
	k := KStarEff(depth)
	return 1.0 - float64(k)/float64(BaseBudget)
}
