// Package text holds helpers the fuzz targets exercise.
package text

// SecondByte returns the second byte of text, which indexes without checking: a string
// shorter than two bytes panics.
func SecondByte(text string) byte {
	return text[1]
}

// Even reports whether data has an even length.
func Even(data []byte) bool {
	return len(data)%2 == 0
}
