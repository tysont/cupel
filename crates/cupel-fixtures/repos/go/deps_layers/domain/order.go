package domain

import "example.com/ledger/deps_layers/infra"

type Order struct {
	Store infra.Store
}
