;; ABOUTME: The registry renders a button through the lazily loaded widget namespace.
;; ABOUTME: It requires only the registry.
(ns shop.registry-test
  (:require [clojure.test :refer :all]
            [shop.registry :as registry]))

(deftest renders-a-button
  (is (= "[ ok ]" (registry/render :button "ok"))))
