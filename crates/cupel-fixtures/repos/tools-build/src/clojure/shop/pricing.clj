;; ABOUTME: Prices for the tools-build fixture: a discount, and cents through the Java helper the build compiles into
;; ABOUTME: target/classes, so loading it without the build is a ClassNotFoundException, as clj-yaml's is.
(ns shop.pricing
  (:import (shop Rounding)))

(defn discount
  "The price after `percent` off."
  [price percent]
  (- price (quot (* price percent) 100)))

(defn in-cents
  "The price in whole cents."
  [price]
  (Rounding/toCents price))
