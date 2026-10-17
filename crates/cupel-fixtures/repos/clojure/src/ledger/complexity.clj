(ns ledger.complexity
  "Functions over the complexity limits.")

(defn label
  "Names an amount by the first rule it matches."
  [amount limit]
  (cond
    (< amount 0) "refund"
    (= amount 0) "empty"
    (> amount (* limit 100)) "huge"
    (> amount (* limit 10)) "large"
    (> amount limit) "over"
    (= amount limit) "at-limit"
    (> amount (/ limit 2)) "high"
    (> amount (/ limit 4)) "medium"
    (> amount (/ limit 10)) "low"
    (> amount 1) "tiny"
    :else "unit"))

(defn settle
  "Applies positive amounts of non-empty batches until the limit is reached."
  [batches limit]
  (let [settled (atom 0)]
    (doseq [batch batches]
      (when (seq batch)
        (doseq [amount batch]
          (when (pos? amount)
            (while (< @settled limit)
              (swap! settled + amount))))))
    @settled))

(defn open-account
  "Describes a new account."
  [owner currency limit overdraft branch opened]
  (format "%s/%s/%d/%b/%d/%d" owner currency limit overdraft branch opened))

(defn statement-template
  "The fixed lines of a printed statement."
  []
  ["ACME LEDGER"
   "==========="
   ""
   "Account holder:"
   "Account number:"
   "Branch:"
   "Currency:"
   "Statement period:"
   ""
   "Opening balance"
   "---------------"
   "Balance brought forward:"
   "Pending credits:"
   "Pending debits:"
   ""
   "Credits"
   "-------"
   "Deposits:"
   "Transfers in:"
   "Interest:"
   "Refunds:"
   "Adjustments:"
   ""
   "Debits"
   "------"
   "Withdrawals:"
   "Transfers out:"
   "Card payments:"
   "Direct debits:"
   "Standing orders:"
   "Fees:"
   "Charges:"
   ""
   "Closing balance"
   "---------------"
   "Balance carried forward:"
   "Available balance:"
   "Overdraft limit:"
   "Overdraft used:"
   ""
   "Notices"
   "-------"
   "Rates changed this period."
   "Fees are listed above."
   "Keep this statement for your records."
   "Report errors within thirty days."
   ""
   "Contact"
   "-------"
   "Telephone:"
   "Email:"
   "Post:"
   ""
   "Opening hours"
   "-------------"
   "Weekdays: 9 to 5"
   "Saturday: 9 to 12"
   "Sunday: closed"
   ""
   "Page 1 of 1"
   "End of statement"])

(defn net
  "The difference between credits and debits."
  [credits debits]
  (- credits debits))

(defrecord Statement [owner])

(defprotocol Printed
  (line [statement amount]))

(extend-type Statement
  Printed
  (line [statement amount]
    (str (:owner statement) ": " amount)))

(defn month
  "Names a month by its number: one case, however many arms it has."
  [number]
  (case number
    1 "jan"
    2 "feb"
    3 "mar"
    4 "apr"
    5 "may"
    6 "jun"
    7 "jul"
    8 "aug"
    9 "sep"
    10 "oct"
    11 "nov"
    12 "dec"
    "none"))
