(ns ledger.report
  "Formats amounts for printing.")

(defn render
  "Cents as a currency amount."
  [cents]
  (format "%d.%02d" (quot cents 100) (rem cents 100)))
