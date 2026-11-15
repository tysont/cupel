// ABOUTME: Which servers of a configuration may vote, looked up by ID.
// ABOUTME: The fixture's tests check it through guarded failure calls (ADR-221).
package votes

import "errors"

// Server is one member of a configuration.
type Server struct {
	ID    string
	Voter bool
}

// HasVote reports whether the server with this ID is a voter.
func HasVote(servers []Server, id string) bool {
	for _, server := range servers {
		if server.ID == id {
			return server.Voter
		}
	}
	return false
}

// Lookup finds a server by ID.
func Lookup(servers []Server, id string) (Server, bool) {
	for _, server := range servers {
		if server.ID == id {
			return server, true
		}
	}
	return Server{}, false
}

// Parse reads one server from its ID, which must not be empty.
func Parse(id string) (Server, error) {
	if id == "" {
		return Server{}, errors.New("empty ID")
	}
	return Server{ID: id, Voter: true}, nil
}
