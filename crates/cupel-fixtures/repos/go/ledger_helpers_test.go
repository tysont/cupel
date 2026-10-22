package ledger

import (
	"reflect"
	"testing"
)

func expectBalance(entries []int64, want int64, t *testing.T) {
	t.Helper()
	if got := Balance(entries); got != want {
		t.Errorf("Balance(%v) = %d, want %d", entries, got, want)
	}
}

func TestAHelperHandedTheTestAsserts(t *testing.T) {
	expectBalance([]int64{2, 3}, 5, t)
}

func TestEachBalanceIsListed(t *testing.T) {
	if got := []int64{Balance([]int64{1, 2})}; !reflect.DeepEqual(got, []int64{3}) {
		t.Errorf("balances = %v", got)
	}
}

func TestABalanceCheckedInASwitch(t *testing.T) {
	switch got := Balance([]int64{1, 2}); {
	case got != 3:
		t.Errorf("balance = %d, want 3", got)
	}
}
