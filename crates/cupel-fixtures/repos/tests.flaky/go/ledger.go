// Package ledger keeps one balance in a package variable, which its tests share.
package ledger

// Balance is the running total of every deposit.
var Balance int

// Deposit adds amount to the balance.
func Deposit(amount int) {
	Balance += amount
}
