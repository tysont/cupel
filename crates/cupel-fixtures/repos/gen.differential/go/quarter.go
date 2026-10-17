// Package quarter splits amounts of cents.
package quarter

// Quarter is a quarter of cents, rounded down.
func Quarter(cents int64) int64 {
	q := cents / 4
	if cents%4 < 0 {
		q--
	}
	return q
}
