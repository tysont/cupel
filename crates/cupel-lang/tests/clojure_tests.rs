// ABOUTME: The Clojure TestFrames producer for assertion libraries over clojure.test: metosin/testit's `fact` and
// ABOUTME: `facts` assert with each arrow they hold, classified by the arrow (P9.9 group 3, cljfx).
mod support;

use cupel_core::syntax::{AssertionKind, AssertionStrength, TestFrames};
use support::Skip::Runs;
use support::{assert_framework, case, produce, rows, tree};

use AssertionKind::{Equality, Throws};
use AssertionStrength::{Strong, Weak};

/// testit's arrows, each an `is` of clojure.test: `=>` and `=eventually=>` compare with `=` (or
/// call a predicate), `=not=>` negates, `=in=>` and `=eventually-in=>` match part of the value,
/// and `=throws=>` expects an exception.
const TESTIT: &str = r#"(ns ledger.core-test
  (:require [clojure.test :refer :all]
            [testit.core :refer :all]
            [ledger.core :as core]))

(deftest totals-are-sums
  (let [total (core/total [1 2])
        _ (fact total => 3)]
    (facts (core/total []) => 0
           (core/total [1]) =not=> 2)))

(deftest a-ledger-holds-its-entries
  (fact (core/ledger [1]) =in=> {:entries [1]})
  (fact (core/later 1) =eventually=> 1)
  (fact (core/later {:a 1}) =eventually-in=> {:a 1}))

(deftest negative-entries-throw
  (fact (core/total [-1]) =throws=> (ex-info? "negative" {})))

(deftest totals-print
  (println (core/total [1])))
"#;

#[test]
fn testit_facts_assert_with_each_arrow() {
    let dir = tree(&[("test/ledger/core_test.clj", TESTIT)]);
    let frames: TestFrames = produce("clojure", dir.path());
    let file = &assert_framework(&frames, "test/ledger/core_test.clj", "clojure-test");
    assert_eq!(
        rows(file),
        [
            case(
                "totals-are-sums",
                &[(Equality, Strong), (Equality, Strong), (Equality, Weak)],
                Runs,
                &[],
                6
            ),
            case(
                "a-ledger-holds-its-entries",
                &[(Equality, Weak), (Equality, Strong), (Equality, Weak)],
                Runs,
                &[],
                12
            ),
            case("negative-entries-throw", &[(Throws, Strong)], Runs, &[], 17),
            case("totals-print", &[], Runs, &[], 20),
        ]
    );
}
