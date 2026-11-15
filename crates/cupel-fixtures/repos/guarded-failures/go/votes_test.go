// ABOUTME: Tests failing under a predicate and a negated flag, which are strong, and one failing only when
// ABOUTME: parsing errs, which stays a weak presence check (ADR-221).
package votes

import "testing"

var servers = []Server{{ID: "id0", Voter: false}, {ID: "id1", Voter: true}}

func TestANonvoterHasNoVote(t *testing.T) {
	if HasVote(servers, "id0") {
		t.Fatalf("id0 has a vote")
	}
}

func TestLookupFindsAServer(t *testing.T) {
	if _, ok := Lookup(servers, "id1"); !ok {
		t.Fatal("id1 is missing")
	}
}

func TestParseAcceptsAnID(t *testing.T) {
	if _, err := Parse("id2"); err != nil {
		t.Fatal(err)
	}
}
