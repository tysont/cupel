// ABOUTME: An exported function that only an unchanged file imports; the head commit edits it, so a
// ABOUTME: range's scope holds this package's file and not its importer (ADR-144).
package prices

// Price is what an item costs.
func Price(cents int) int {
	return cents * 2
}
