(ns ledger.main
  "Prints a statement for the current user."
  (:require [ledger.clones-settlement]
            [ledger.complexity]
            [ledger.core :as core]
            [ledger.decide]
            [ledger.pricing]
            [ledger.twins])
  (:gen-class))

(defn -main [& _]
  (let [account (core/deposit (core/open-account (System/getenv "USER")) 1250)]
    (println (core/statement account))))
