package port

import "example.com/ledger/deps_layers/domain"

type Port interface {
	Place(order domain.Order)
}
