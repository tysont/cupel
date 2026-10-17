(ns discount.small-test
  "A small order: no discount; never the boundary of exactly 100."
  (:require [clojure.test :refer [deftest is]]
            [discount.core :as discount]))

(deftest keeps-a-small-order
  (is (= 50 (discount/discount 50 true))))
