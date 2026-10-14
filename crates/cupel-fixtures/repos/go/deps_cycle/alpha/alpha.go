package alpha

import "example.com/ledger/deps_cycle/beta"

const Alpha = beta.Beta + 1
