package ledger

// Pairs of functions that differ in one way, for structural-hash conformance.

func netTotal(items []int64, fee int64) int64 {
	gross := sum(items)
	if gross > fee {
		return gross - fee
	}
	return 0
}

func sumAfterCharge(values []int64, charge int64) int64 {
	subtotal := sum(values)
	if subtotal > charge {
		return subtotal - charge
	}
	return 0
}

func netTotalFloor(items []int64, fee int64) int64 {
	gross := sum(items)
	if gross > fee {
		return gross - fee
	}
	return 100
}

func netTotalLaidOut(items []int64, fee int64) int64 {
	// The same body as netTotal, laid out differently.
	gross := sum(
		items,
	)

	if gross > fee {
		return gross - fee // what is left after the fee
	}
	return 0
}

func netTotalPlus(items []int64, fee int64) int64 {
	gross := sum(items)
	if gross > fee {
		return gross + fee
	}
	return 0
}

func netTotalCapped(items []int64, fee int64) int64 {
	gross := sum(items)
	if gross > fee {
		return gross - fee
	} else if gross < 0 {
		return gross
	}
	return 0
}

func netTotalProduct(items []int64, fee int64) int64 {
	gross := product(items)
	if gross > fee {
		return gross - fee
	}
	return 0
}

func sum(items []int64) int64 {
	var total int64
	for _, item := range items {
		total += item
	}
	return total
}

func product(items []int64) int64 {
	total := int64(1)
	for _, item := range items {
		total *= item
	}
	return total
}
