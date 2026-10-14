package ledger

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestMembersPayExactlyTheDiscountedPrice(t *testing.T) {
	assert.Equal(t, int64(900), discounted(1000, 10, true))
}

func TestBlankAmountsAreRejected(t *testing.T) {
	_, err := parseCents("")
	assert.ErrorIs(t, err, ErrEmpty)
}

func TestStatementsAreNotEmpty(t *testing.T) {
	assert.NotEmpty(t, NewAccount("ann").Statement())
}

func TestClassifyingRuns(t *testing.T) {
	classify(100, 50)
}

func TestMailGoesThroughEveryNotifier(t *testing.T) {
	primary := &MockNotifier{}
	backup := &MockNotifier{}
	audit := &MockNotifier{}
	assert.Nil(t, Mail(primary, NewAccount("bo")))
	_, _ = backup, audit
}

func TestEveryEntryCountsTowardTheBalance(t *testing.T) {
	for _, cents := range []int64{1, 20, 300} {
		assert.Equal(t, cents, Balance([]int64{cents}))
	}
}

func TestDepositsKeepTheOwner(t *testing.T) {
	account := NewAccount("kim")
	account.Deposit(100)
	account.Deposit(50)
	assert.Equal(t, "kim", account.Owner)
}

func TestDepositsAccumulate(t *testing.T) {
	account := NewAccount("lee")
	account.Deposit(200)
	account.Deposit(25)
	total := account.cents
	assert.Equal(t, int64(225), total)
}
