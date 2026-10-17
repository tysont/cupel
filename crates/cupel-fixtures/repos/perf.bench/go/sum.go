package units

// Sum adds the values.
func Sum(values []int) int {
	total := 0
	for _, value := range values {
		total += value
	}
	return total
}
