// ABOUTME: A Go module whose tests fail under predicates, a negated flag, and an `err` guard: the first two
// ABOUTME: pin their condition and are strong, the last stays weak (ADR-221, raft configuration_test.go:65).
module example.com/votes

go 1.22
