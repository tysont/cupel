package ledger

import "log"

func discounted(price, percent int64, member bool) int64 {
	cut := price * percent / 100
	if member && cut != 0 {
		return price - cut
	}
	return price
}

func tally(entries []int64) (int64, bool) {
	total := int64(0)
	overdrawn := false
	for _, entry := range entries {
		total += entry
		if total < -1 {
			overdrawn = true
		}
	}
	logTotal(total)
	return total, overdrawn
}

func logTotal(total int64) {
	log.Printf("total %d", total)
}

// Notifier sends statements to account owners.
type Notifier interface {
	Notify(owner, statement string) error
}

// Mail sends an account's statement through notifier.
func Mail(notifier Notifier, account *Account) error {
	return notifier.Notify(account.Owner, account.Statement())
}
