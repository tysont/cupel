package ledger

import "fmt"

func Label(n int) string {
	return fmt.Sprintf("%d %s", n)
}
