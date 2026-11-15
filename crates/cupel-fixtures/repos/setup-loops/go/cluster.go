// ABOUTME: A cluster configuration of servers, encoded and decoded as a list of addresses, whose tests
// ABOUTME: build their servers in loops (raft's configuration_test.go:301, Q-go-1).
package cluster

import "strings"

// Server is one member of a cluster.
type Server struct {
	ID      string
	Voter   bool
	Address string
}

// Encode joins the servers' addresses.
func Encode(servers []Server) string {
	addresses := make([]string, 0, len(servers))
	for _, server := range servers {
		addresses = append(addresses, server.Address)
	}
	return strings.Join(addresses, ",")
}

// Decode splits addresses into voting servers named by their address.
func Decode(text string) []Server {
	var servers []Server
	for _, address := range strings.Split(text, ",") {
		servers = append(servers, Server{ID: address, Voter: true, Address: address})
	}
	return servers
}
