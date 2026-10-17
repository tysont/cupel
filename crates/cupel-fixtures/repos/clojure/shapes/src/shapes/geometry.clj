(ns shapes.geometry
  "Areas of plane figures.")

(defn square
  "The area of a square with sides of `side`."
  [side]
  (* side side))

(defprotocol Figure
  (area [figure]))

(defrecord Circle [radius]
  Figure
  (area [this]
    (* Math/PI radius radius)))

(defn- scaled [circle factor]
  (let [times (fn [value] (* value factor))]
    (->Circle (times (:radius circle)))))
