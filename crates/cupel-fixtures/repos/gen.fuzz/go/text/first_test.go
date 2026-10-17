package text

import "testing"

func FuzzSecondByte(f *testing.F) {
	f.Fuzz(func(t *testing.T, text string) {
		SecondByte(text)
	})
}

func FuzzEven(f *testing.F) {
	f.Fuzz(func(t *testing.T, data []byte) {
		if Even(data) != (len(data)%2 == 0) {
			t.Errorf("Even(%d bytes) is wrong", len(data))
		}
	})
}
