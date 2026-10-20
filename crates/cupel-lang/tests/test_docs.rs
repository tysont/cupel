// ABOUTME: Each language's TestFrames carries a test case's doc comment or docstring as written, and only the
// ABOUTME: language's documentation form directly above (or, in Python, atop) the test (ADR-134).
mod support;

use cupel_core::syntax::TestFrames;
use support::{file, produce, tree};

/// (name, doc) for each case of `path` in a tree of `files`, produced as `lang`.
fn docs(lang: &str, files: &[(&str, &str)], path: &str) -> Vec<(String, Option<String>)> {
    let dir = tree(files);
    let frames: TestFrames = produce(lang, dir.path());
    file(&frames, path)
        .cases
        .iter()
        .map(|case| (case.name.clone(), case.doc.clone()))
        .collect()
}

fn row(name: &str, doc: Option<&str>) -> (String, Option<String>) {
    (name.to_owned(), doc.map(str::to_owned))
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

#[test]
fn rust_reads_outer_doc_lines_and_blocks_above_the_attributes() {
    assert_eq!(
        docs("rust", &[("tests/dates.rs", RUST)], "tests/dates.rs"),
        [
            row(
                "parses_week_dates",
                Some("/// Parses week dates.\n/// Traces REQ-1.")
            ),
            row("plain", None),
            row("block", Some("/** Traces REQ-2. */")),
            row("after_a_documented_helper", None),
        ]
    );
}

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

#[test]
fn go_reads_the_comment_lines_directly_above_the_function() {
    assert_eq!(
        docs("go", &[("dates_test.go", GO)], "dates_test.go"),
        [
            row(
                "TestParses",
                Some("// TestParses parses week dates.\n// It traces REQ-1.")
            ),
            row("TestPlain", None),
        ]
    );
}

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

#[test]
fn python_reads_the_docstring() {
    assert_eq!(
        docs("python", &[("tests/test_dates.py", PYTHON)], "tests/test_dates.py"),
        [
            row("test_parses", Some("\"\"\"Parses week dates (REQ-1).\"\"\"")),
            row("test_plain", None),
            row("test_method", Some("'''Traces REQ-2.'''")),
        ]
    );
}

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

#[test]
fn javascript_and_typescript_read_the_jsdoc_block_above_the_call() {
    for (lang, path) in [
        ("javascript", "test/dates.test.js"),
        ("typescript", "src/dates.test.ts"),
    ] {
        assert_eq!(
            docs(lang, &[(path, ECMA)], path),
            [
                row("parses", Some("/** Parses week dates (REQ-1). */")),
                row("plain", None),
                row("at the top level", Some("/**\n * Traces REQ-2.\n */")),
            ],
            "{lang}"
        );
    }
}

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

#[test]
fn java_reads_the_javadoc_above_the_method() {
    let path = "src/test/java/dates/DatesTest.java";
    assert_eq!(
        docs("java", &[(path, JAVA)], path),
        [
            row("parses", Some("/** Parses week dates (REQ-1). */")),
            row("plain", None),
        ]
    );
}

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

#[test]
fn clojure_reads_the_comment_lines_above_the_form() {
    let path = "test/dates/core_test.clj";
    assert_eq!(
        docs("clojure", &[(path, CLOJURE)], path),
        [
            row("parses", Some(";; Parses week dates.\n;; Traces REQ-1.")),
            row("plain", None),
        ]
    );
}
