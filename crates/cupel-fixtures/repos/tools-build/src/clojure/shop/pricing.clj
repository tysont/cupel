;; ABOUTME: Prices for the tools-build fixture: a discount the tests cover, beside the Java helper the build
;; ABOUTME: compiles into target/classes.
(ns shop.pricing)

(defn discount
  "The price after `percent` off."
  [price percent]
  (- price (quot (* price percent) 100)))
