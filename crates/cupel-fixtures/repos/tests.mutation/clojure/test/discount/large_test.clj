(ns discount.large-test
  "A large member order: the discount applies."
  (:require [clojure.test :refer [deftest is]]
            [discount.core :as discount]))

(deftest discounts-a-large-member-order
  (is (= 190 (discount/discount 200 true))))
