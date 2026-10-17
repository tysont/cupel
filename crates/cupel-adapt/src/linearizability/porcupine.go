// ABOUTME: Cupel's Porcupine runner (ADR-119): reads register histories split into independent partitions as JSON on
// ABOUTME: standard input, checks each partition's linearizability, and prints each result with its longest prefix.
//
// Input:  {"timeout_ms": N, "histories": [[[op, ...], ...], ...]}, a history being a list of partitions.
// An op:  {"f": "read"|"write"|"cas", "read"|"write"|"from"|"to": "<JSON>", "call": n, "return": n}
// Output: {"histories": [[{"result": "Ok"|"Illegal"|"Unknown", "longest": [id, ...], "states": ["<JSON>", ...]}]]}
//
// Values are canonical JSON text, so a register's state is the text of its value, `null` at first. An op whose
// outcome is unknown returns after every other op, so it may take effect anywhere after its call or not at all; a
// read of unknown outcome observed nothing and is not sent.
package main

import (
	"encoding/json"
	"fmt"
	"hash/fnv"
	"os"
	"time"

	"github.com/anishathalye/porcupine"
)

type op struct {
	F      string `json:"f"`
	Read   string `json:"read"`
	Write  string `json:"write"`
	From   string `json:"from"`
	To     string `json:"to"`
	Call   int64  `json:"call"`
	Return int64  `json:"return"`
}

type request struct {
	TimeoutMs int64    `json:"timeout_ms"`
	Histories [][][]op `json:"histories"`
}

type result struct {
	Result  string   `json:"result"`
	Longest []int    `json:"longest"`
	States  []string `json:"states"`
}

type response struct {
	Histories [][]result `json:"histories"`
}

// next is the state a register in state is in after o, and whether o can happen there.
func next(state string, o op) (bool, string) {
	switch o.F {
	case "read":
		return o.Read == state, state
	case "write":
		return true, o.Write
	case "cas":
		return o.From == state, o.To
	}
	return false, state
}

var register = porcupine.Model{
	Init: func() interface{} { return "null" },
	Step: func(state, input, _ interface{}) (bool, interface{}) {
		return next(state.(string), input.(op))
	},
	Hash: func(state interface{}) uint64 {
		h := fnv.New64a()
		h.Write([]byte(state.(string)))
		return h.Sum64()
	},
}

// longest is the longest of partials, the lexicographically least among equals, so the
// excerpt is the same on every run.
func longest(partials [][]int) []int {
	best := []int{}
	for _, partial := range partials {
		if len(partial) > len(best) || (len(partial) == len(best) && less(partial, best)) {
			best = partial
		}
	}
	return best
}

func less(a, b []int) bool {
	for i := range a {
		if a[i] != b[i] {
			return a[i] < b[i]
		}
	}
	return false
}

// after is the state the register is in once the ops of prefix took effect in order.
func after(ops []op, prefix []int) string {
	state := "null"
	for _, id := range prefix {
		_, state = next(state, ops[id])
	}
	return state
}

func check(ops []op, timeout time.Duration) result {
	history := make([]porcupine.Operation, len(ops))
	for i, o := range ops {
		history[i] = porcupine.Operation{Input: o, Call: o.Call, Return: o.Return}
	}
	outcome, info := porcupine.CheckOperationsVerbose(register, history, timeout)
	checked := result{Result: string(outcome), Longest: []int{}, States: []string{}}
	if outcome == porcupine.Illegal {
		partials := info.PartialLinearizations()
		if len(partials) > 0 {
			checked.Longest = longest(partials[0])
		}
		checked.States = []string{after(ops, checked.Longest)}
	}
	return checked
}

func main() {
	var req request
	if err := json.NewDecoder(os.Stdin).Decode(&req); err != nil {
		fmt.Fprintf(os.Stderr, "cannot read the histories: %v\n", err)
		os.Exit(2)
	}
	timeout := time.Duration(req.TimeoutMs) * time.Millisecond
	resp := response{Histories: make([][]result, len(req.Histories))}
	for i, partitions := range req.Histories {
		resp.Histories[i] = make([]result, len(partitions))
		for j, ops := range partitions {
			resp.Histories[i][j] = check(ops, timeout)
		}
	}
	if err := json.NewEncoder(os.Stdout).Encode(resp); err != nil {
		fmt.Fprintf(os.Stderr, "cannot print the results: %v\n", err)
		os.Exit(2)
	}
}
