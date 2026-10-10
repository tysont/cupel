package ledger

import (
	"errors"
	"strconv"
	"strings"
)

// ErrEmpty reports a blank amount.
var ErrEmpty = errors.New("empty amount")

func classify(amount, limit int64) string {
	kind := "credit"
	if amount < 0 {
		kind = "debit"
	} else if amount > limit && limit > 0 {
		kind = "large"
	}
	return kind
}

func firstOverdraft(balances []int64) int {
	for index, balance := range balances {
		if balance < 0 {
			return index
		}
	}
	return -1
}

func parseCents(text string) (int64, error) {
	trimmed := strings.TrimSpace(text)
	if trimmed == "" {
		return 0, ErrEmpty
	}
	return strconv.ParseInt(trimmed, 10, 64)
}

func describe(code int) string {
	switch code {
	case 0:
		return "ok"
	case 1, 2:
		return "retry"
	default:
		return "fail"
	}
}

func kindOf(value any) string {
	switch value.(type) {
	case int, int64:
		return "number"
	case string:
		return "text"
	}
	return "other"
}

func drain(entries <-chan int64, done <-chan struct{}) int64 {
	var total int64
	for {
		select {
		case entry := <-entries:
			total += entry
		case <-done:
			return total
		}
	}
}
