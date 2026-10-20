// ABOUTME: Each language's TestFrames carries a test case's doc comment or docstring as written, and only the
// ABOUTME: language's documentation form directly above (or, in Python, atop) the test (ADR-134).
mod support;

use cupel_core::syntax::TestFrames;
use support::{file, produce, tree};

/// A test file in one language and the doc each of its cases should carry, by name.
struct Documented {
    lang: &'static str,
    path: &'static str,
    source: &'static str,
    docs: &'static [(&'static str, Option<&'static str>)],
}

const RUST: &str = "\
/// Parses week dates.
/// Traces REQ-1.
#[test]
fn parses_week_dates() {
    assert_eq!(1, 1);
}

// A plain comment is not documentation.
#[test]
fn plain() {
    assert_eq!(1, 1);
}

/** Traces REQ-2. */
#[test]
#[ignore]
fn block() {
    assert_eq!(1, 1);
}

/// A helper, not a test.
fn helper() {}

#[test]
fn after_a_documented_helper() {
    helper();
}
";

const GO: &str = "\
package dates

import \"testing\"

// TestParses parses week dates.
// It traces REQ-1.
func TestParses(t *testing.T) {
	if 1 != 1 {
		t.Fatal(\"no\")
	}
}

func TestPlain(t *testing.T) {
	if 1 != 1 {
		t.Fatal(\"no\")
	}
}
";

const PYTHON: &str = "\
def test_parses():
    \"\"\"Parses week dates (REQ-1).\"\"\"
    assert 1 == 1


def test_plain():
    # A comment is not a docstring.
    assert 1 == 1


class TestDates:
    def test_method(self):
        '''Traces REQ-2.'''
        assert 1 == 1
";

const ECMA: &str = "\
describe(\"dates\", () => {
  /** Parses week dates (REQ-1). */
  it(\"parses\", () => {
    expect(1).toBe(1);
  });

  // A line comment is not documentation.
  it(\"plain\", () => {
    expect(1).toBe(1);
  });
});

/**
 * Traces REQ-2.
 */
test(\"at the top level\", () => {
  expect(1).toBe(1);
});
";

const JAVA: &str = "\
package dates;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.assertEquals;

class DatesTest {
    /** Parses week dates (REQ-1). */
    @Test
    void parses() {
        assertEquals(1, 1);
    }

    // A line comment is not documentation.
    @Test
    void plain() {
        assertEquals(1, 1);
    }
}
";

const CLOJURE: &str = "\
(ns dates.core-test
  (:require [clojure.test :refer [deftest is]]))

;; Parses week dates.
;; Traces REQ-1.
(deftest parses
  (is (= 1 1)))

(deftest plain
  (is (= 1 1)))
";

const DOCUMENTED: [Documented; 7] = [
    Documented {
        lang: "rust",
        path: "tests/dates.rs",
        source: RUST,
        docs: &[
            (
                "parses_week_dates",
                Some("/// Parses week dates.\n/// Traces REQ-1."),
            ),
            ("plain", None),
            ("block", Some("/** Traces REQ-2. */")),
            ("after_a_documented_helper", None),
        ],
    },
    Documented {
        lang: "go",
        path: "dates_test.go",
        source: GO,
        docs: &[
            (
                "TestParses",
                Some("// TestParses parses week dates.\n// It traces REQ-1."),
            ),
            ("TestPlain", None),
        ],
    },
    Documented {
        lang: "python",
        path: "tests/test_dates.py",
        source: PYTHON,
        docs: &[
            (
                "test_parses",
                Some("\"\"\"Parses week dates (REQ-1).\"\"\""),
            ),
            ("test_plain", None),
            ("test_method", Some("'''Traces REQ-2.'''")),
        ],
    },
    Documented {
        lang: "javascript",
        path: "test/dates.test.js",
        source: ECMA,
        docs: ECMA_DOCS,
    },
    Documented {
        lang: "typescript",
        path: "src/dates.test.ts",
        source: ECMA,
        docs: ECMA_DOCS,
    },
    Documented {
        lang: "java",
        path: "src/test/java/dates/DatesTest.java",
        source: JAVA,
        docs: &[
            ("parses", Some("/** Parses week dates (REQ-1). */")),
            ("plain", None),
        ],
    },
    Documented {
        lang: "clojure",
        path: "test/dates/core_test.clj",
        source: CLOJURE,
        docs: &[
            ("parses", Some(";; Parses week dates.\n;; Traces REQ-1.")),
            ("plain", None),
        ],
    },
];

const ECMA_DOCS: &[(&str, Option<&str>)] = &[
    ("parses", Some("/** Parses week dates (REQ-1). */")),
    ("plain", None),
    ("at the top level", Some("/**\n * Traces REQ-2.\n */")),
];

/// Rust's `///` lines and `/** */` above the attributes, Go's `//` lines, Python's docstring,
/// `/** */` above the `it`/`test` statement and above a Java method, and Clojure's `;` lines; a
/// plain comment, or a doc on a function that is not a test, documents no case.
#[test]
fn each_language_reads_its_own_doc_form_above_a_test() {
    for documented in &DOCUMENTED {
        let dir = tree(&[(documented.path, documented.source)]);
        let frames: TestFrames = produce(documented.lang, dir.path());
        let docs: Vec<(&str, Option<&str>)> = file(&frames, documented.path)
            .cases
            .iter()
            .map(|case| (case.name.as_str(), case.doc.as_deref()))
            .collect();
        assert_eq!(docs, documented.docs, "{}", documented.lang);
    }
}
