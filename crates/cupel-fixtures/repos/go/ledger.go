package ledger

import (
	"fmt"

	"github.com/google/uuid"

	"example.com/ledger/gen"
	"example.com/ledger/report"
	"example.com/shapes"
)

// MaxEntries bounds a single statement.
const MaxEntries = 100

// Currency is the ledger's display currency.
var Currency = "USD"

var registry map[string]*Account

func init() {
	registry = make(map[string]*Account)
}

// Account holds a running balance in cents.
type Account struct {
	ID    string
	Owner string
	cents int64
}

// Audited is implemented by anything with an audit trail.
type Audited interface {
	AuditTrail() []string
}

// NewAccount opens an account for owner.
func NewAccount(owner string) *Account {
	account := &Account{ID: uuid.NewString(), Owner: owner}
	registry[account.ID] = account
	return account
}

// Deposit adds cents to the balance.
func (a *Account) Deposit(cents int64) {
	a.cents += cents
}

// Statement renders the account for printing.
func (a *Account) Statement() string {
	header := func() string {
		return fmt.Sprintf("%s (%s)", a.Owner, gen.Version)
	}
	return header() + "\n" + report.Render(a.cents)
}

func (a Account) audited() bool {
	return a.cents >= 0
}

// Balance sums entries.
func Balance(entries []int64) int64 {
	return scale(entries, 1)
}

func scale(entries []int64, factor int64) int64 {
	var total int64
	for _, entry := range entries {
		total += entry * factor
	}
	return total
}

// FloorArea is the area of a square room.
func FloorArea(side float64) float64 {
	return shapes.Square(side)
}
