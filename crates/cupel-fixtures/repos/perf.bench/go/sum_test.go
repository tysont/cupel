package units

import "testing"

func BenchmarkSum(b *testing.B) {
	values := make([]int, 100)
	for i := 0; i < b.N; i++ {
		Sum(values)
	}
}
