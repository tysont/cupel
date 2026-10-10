package report

import "fmt"

// Render formats cents as a currency amount.
func Render(cents int64) string {
	return fmt.Sprintf("%d.%02d", cents/100, cents%100)
}
