(ns ledger.settle-test
  (:require [clojure.java.io :as io]
            [clojure.test :refer [deftest is]]))

(deftest settles-on-every-other-run
  ;; Fails on every other run: state outside the process flips its outcome between identical runs.
  (let [marker (io/file ".settled")]
    (if (.exists marker)
      (do (io/delete-file marker)
          (is false "settled twice in a row"))
      (spit marker ""))))
