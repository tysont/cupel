(ns ledger.dead-export
  "Totals nobody asks for.")

(defn unused-total
  "How many entries there are."
  [entries]
  (count entries))
