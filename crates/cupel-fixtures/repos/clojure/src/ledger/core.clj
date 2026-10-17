(ns ledger.core
  "The ledger: accounts by ID and the arithmetic over their entries."
  (:require [cheshire.core :as json]
            [clojure.string :as str]
            [ledger.gen :as gen]
            [ledger.report :as report]
            [shapes.geometry :as geometry])
  (:import (java.util UUID)))

(def ^:const max-entries
  "Bounds a single statement."
  100)

(def currency
  "The ledger's display currency."
  "USD")

(def registry (atom {}))

(defprotocol Audited
  (audit-trail [account]))

(defrecord Account [id owner cents]
  Audited
  (audit-trail [this]
    [id]))

(defn open-account
  "Opens an account for owner."
  [owner]
  (let [account (->Account (str (UUID/randomUUID)) owner 0)]
    (swap! registry assoc (:id account) account)
    account))

(defn deposit
  "Adds cents to the balance."
  [account cents]
  (update account :cents + cents))

(defn- scale [entries factor]
  (reduce + (map #(* % factor) entries)))

(defn balance
  "Sums entries."
  [entries]
  (scale entries 1))

(defn floor-area
  "The area of a square room."
  [side]
  (geometry/square side))

(defn statement
  "The account rendered for printing."
  [account]
  (letfn [(header [owner]
            (str owner " (" gen/version ")"))]
    (str (header (:owner account)) "\n" (report/render (:cents account)))))

(defn as-json
  "The account as JSON."
  [account]
  (let [title (fn [owner] (str/upper-case owner))]
    (json/generate-string (assoc account :owner (title (:owner account))))))
