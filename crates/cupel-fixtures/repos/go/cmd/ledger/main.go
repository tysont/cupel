package main

import (
	"fmt"
	"os"

	"example.com/ledger"
)

func main() {
	account := ledger.NewAccount(os.Getenv("USER"))
	account.Deposit(1250)
	fmt.Println(account.Statement())
}
