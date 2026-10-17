(ns counter.core
  "A counter that logs each step it takes to a trace file, when it has one.")

(defn- log!
  [{:keys [trace count done]} action]
  (when trace
    (spit trace
          (str "{" (when action (str "\"action\": \"" action "\", "))
               "\"state\": {\"count\": " count ", \"done\": " done "}}\n")
          :append true)))

(defn counter
  "A counter at zero, logging to trace (a path, or nil)."
  [trace]
  (let [c {:trace trace :count 0 :done false}]
    (log! c nil)
    c))

(defn inc-count
  "Counts one up."
  [c]
  (let [c (update c :count + 1)]
    (log! c "Inc")
    c))

(defn bump
  "Counts two up, though it logs one step."
  [c]
  (let [c (update c :count + 2)]
    (log! c "Inc")
    c))

(defn finish
  "Stops counting."
  [c]
  (let [c (assoc c :done true)]
    (log! c "Finish")
    c))
