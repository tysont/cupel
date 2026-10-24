// ABOUTME: The one importer of `prices.Price`, which the head commit leaves unchanged.
// ABOUTME: Its import is the only thing that keeps the `prices` package live.
package main

import (
	"fmt"

	"example.com/shop/prices"
)

func main() {
	fmt.Println(prices.Price(1))
}
