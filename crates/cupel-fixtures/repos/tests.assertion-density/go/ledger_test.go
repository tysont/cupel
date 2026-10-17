package ledger

import "testing"

// Dense tests: each pins every outcome it exercises.

func TestDepositsAndWithdrawalsAddUp(t *testing.T) {
	if got := Balance(nil); got != 0 {
		t.Errorf("Balance(nil) = %d, want 0", got)
	}
	if got := Balance([]int{5}); got != 5 {
		t.Errorf("Balance(5) = %d, want 5", got)
	}
	if got := Balance([]int{5, -3}); got != 2 {
		t.Errorf("Balance(5, -3) = %d, want 2", got)
	}
	if got := Balance([]int{-3, -4}); got != -7 {
		t.Errorf("Balance(-3, -4) = %d, want -7", got)
	}
}

func TestANegativeBalanceIsOverdrawn(t *testing.T) {
	if !Overdrawn([]int{-1}) {
		t.Error("-1 is not overdrawn")
	}
	if !Overdrawn([]int{5, -6}) {
		t.Error("5, -6 is not overdrawn")
	}
	if Overdrawn(nil) {
		t.Error("nothing is overdrawn")
	}
	if Overdrawn([]int{5, -5}) {
		t.Error("5, -5 is overdrawn")
	}
}

func TestOrderDoesNotChangeTheBalance(t *testing.T) {
	if got := Balance([]int{1, 2, 3}); got != 6 {
		t.Errorf("Balance(1, 2, 3) = %d, want 6", got)
	}
	if got := Balance([]int{3, 2, 1}); got != 6 {
		t.Errorf("Balance(3, 2, 1) = %d, want 6", got)
	}
	if got := Balance([]int{2, -2, 1}); got != 1 {
		t.Errorf("Balance(2, -2, 1) = %d, want 1", got)
	}
	if got := Balance([]int{-2, 1, 2}); got != 1 {
		t.Errorf("Balance(-2, 1, 2) = %d, want 1", got)
	}
}
