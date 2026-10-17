package ledger

import (
	"testing"
	"testing/quick"
)

func TestBalanceOfOneEntryIsTheEntry(t *testing.T) {
	one := func(cents int64) bool { return Balance([]int64{cents}) == cents }
	if err := quick.Check(one, nil); err != nil {
		t.Error(err)
	}
}
