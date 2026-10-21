(ns ledger.references
  "Reference cases for static.dead-code (ADR-135): an exported rate, one only a requiring
  namespace calls, one only a name resolved at run time reaches, and one nothing names.")

(defn exported-rate
  "Exported, so live by convention though nothing calls it."
  [amount]
  (* amount 3))

(defn imported-rate
  "Called only from ledger.rates, which requires this namespace."
  [amount]
  (+ amount amount))

(defn- tiered-rate [amount]
  (if (> amount 100) (quot amount 3) amount))

(defn- orphan-rate [amount]
  (- amount (quot amount 4)))

(defn rate-named
  "Applies the rate kind names, resolved at run time."
  [kind amount]
  ((resolve (symbol "ledger.references" (str kind "-rate"))) amount))
