package ledger

import (
	"os"
	"testing"
)

func TestMain(m *testing.M) {
	registry = make(map[string]*Account)
	os.Exit(m.Run())
}

func TestDepositAddsToTheBalance(t *testing.T) {
	account := NewAccount("ada")
	t.Cleanup(func() { delete(registry, account.ID) })
	account.Deposit(250)
	if got, want := account.cents, int64(250); got != want {
		t.Errorf("balance = %d, want %d", got, want)
	}
}

func TestMembersGetADiscount(t *testing.T) {
	if !(discounted(200, 50, true) < 200) {
		t.Fatal("members should pay less")
	}
}

func TestNewAccountsAreRegistered(t *testing.T) {
	account := registry[NewAccount("ada").ID]
	if account == nil {
		t.Fatal("the account is not registered")
	}
}

func TestParsingGarbageFails(t *testing.T) {
	if _, err := parseCents("12x"); err == nil {
		t.Error("parsed garbage")
	}
}

func TestNewAccountsHaveDistinctIDs(t *testing.T) {
	first, second := NewAccount("ada"), NewAccount("grace")
	if first.ID == second.ID {
		t.Errorf("both accounts have ID %s", first.ID)
	}
}

func TestClassify(t *testing.T) {
	cases := []struct {
		name   string
		amount int64
		want   string
	}{
		{"debit", -5, "debit"},
		{"large", 500, "large"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			if got := classify(tc.amount, 100); got != tc.want {
				t.Errorf("classify(%d) = %q, want %q", tc.amount, got, tc.want)
			}
		})
	}
}

func TestStatementIsMailed(t *testing.T) {
	if testing.Short() {
		t.Skip("talks to the network")
	}
	if statement := NewAccount("ada").Statement(); statement == "" {
		t.Fatal("the statement is empty")
	}
}

func TestParseCentsTrimsSpaces(t *testing.T) {
	cents, err := parseCents(" 42 ")
	if err != nil {
		t.Fatal(err)
	}
	if cents != 42 {
		t.Errorf("cents = %d, want 42", cents)
	}
}

func TestOpeningAnAccountDoesNotPanic(t *testing.T) {
	NewAccount("ada")
}
