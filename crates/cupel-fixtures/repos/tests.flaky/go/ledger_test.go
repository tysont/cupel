package ledger

import "testing"

// Passes only while no test before it has deposited: order-dependent.
func TestBalanceStartsAtZero(t *testing.T) {
	if Balance != 0 {
		t.Fatalf("balance = %d, want 0", Balance)
	}
}

// Leaves the shared balance changed for every test after it.
func TestDepositChangesBalance(t *testing.T) {
	Deposit(5)
	if Balance < 5 {
		t.Fatalf("balance = %d, want at least 5", Balance)
	}
}
