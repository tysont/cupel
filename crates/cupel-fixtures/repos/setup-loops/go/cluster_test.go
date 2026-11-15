// ABOUTME: Three tests with loops: one building its inputs (setup, no test.logic), one deciding what it builds
// ABOUTME: (logic), and one asserting on each pass (table-driven, no logic) (ADR-220).
package cluster

import (
	"fmt"
	"reflect"
	"testing"
)

// A loop that only builds the servers the test then checks once: setup, no logic.
func TestEncodeDecodeRoundTrips(t *testing.T) {
	var servers []Server
	for i := 0; i < 3; i++ {
		address := fmt.Sprintf("10.0.0.%d", i)
		servers = append(servers, Server{ID: address, Voter: true, Address: address})
	}
	decoded := Decode(Encode(servers))
	if !reflect.DeepEqual(servers, decoded) {
		t.Fatalf("decoded %v, want %v", decoded, servers)
	}
}

// A loop that decides what it builds: logic.
func TestDecodeMakesEveryServerAVoter(t *testing.T) {
	var servers []Server
	for i := 0; i < 3; i++ {
		address := fmt.Sprintf("10.0.0.%d", i)
		server := Server{ID: address, Voter: true, Address: address}
		if i == 0 {
			server.Voter = false
		}
		servers = append(servers, server)
	}
	if got := Decode(Encode(servers))[0]; !got.Voter {
		t.Fatalf("%v is not a voter", got)
	}
}

// A loop that asserts on each pass: table-driven, no logic.
func TestDecodeNamesEachServerByItsAddress(t *testing.T) {
	for _, server := range Decode("a,b,c") {
		if server.ID != server.Address {
			t.Errorf("server %q has address %q", server.ID, server.Address)
		}
	}
}
