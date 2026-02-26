package consistency

// ddis:implements APP-ADR-034 (pure-Go tiered consistency — global variable namespace)
// ddis:maintains APP-INV-021 (SAT encoding fidelity — global namespace for cross-invariant UNSAT)

import "fmt"

// VarMap manages a global namespace for propositional variables.
// Two invariants referencing the same predicate (e.g., "render") use the
// SAME variable ID, enabling cross-invariant UNSAT detection.
type VarMap struct {
	nameToID map[string]int
	idToName map[int]string
	next     int
}

// NewVarMap creates a fresh global variable namespace.
func NewVarMap() *VarMap {
	return &VarMap{
		nameToID: make(map[string]int),
		idToName: make(map[int]string),
		next:     1, // gophersat uses 1-based variable IDs
	}
}

// Get returns the integer variable ID for a named variable.
// Creates a new ID if the name is not yet registered.
func (vm *VarMap) Get(name string) int {
	if id, ok := vm.nameToID[name]; ok {
		return id
	}
	id := vm.next
	vm.next++
	vm.nameToID[name] = id
	vm.idToName[id] = name
	return id
}

// Name returns the human-readable name for a variable ID.
func (vm *VarMap) Name(id int) string {
	return vm.idToName[id]
}

// Count returns the total number of registered variables.
func (vm *VarMap) Count() int {
	return vm.next - 1
}

// MakePredicateVar creates a variable name from a predicate and its arguments.
// Example: MakePredicateVar("render", "spec") → "render_spec"
func MakePredicateVar(pred, args string) string {
	return fmt.Sprintf("%s_%s", sanitize(pred), sanitize(args))
}

// MakeDotVar creates a variable name from a dot-path and value.
// Example: MakeDotVar("w.type", "test") → "w_type_eq_test"
func MakeDotVar(path, value string) string {
	return fmt.Sprintf("%s_eq_%s", sanitize(path), sanitize(value))
}
