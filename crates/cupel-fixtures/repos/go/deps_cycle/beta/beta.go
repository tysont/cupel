package beta

import "example.com/ledger/deps_cycle/alpha"

const Beta = 2

var Both = []int{alpha.Alpha, Beta}
