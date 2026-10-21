package ledger

// Doubled doubles amount through a rate of another file of the package.
func Doubled(amount int64) int64 {
	return importedRate(amount)
}
