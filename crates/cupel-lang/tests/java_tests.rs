// ABOUTME: The Java TestFrames producer on hand-written trees: a JUnit test that calls an asserting helper method
// ABOUTME: of its own class asserts as that helper does, and a `try` ending in `fail` asserts its exception.
mod support;

use cupel_core::syntax::{AssertionKind, AssertionStrength, TestFrames};
use support::Skip::Runs;
use support::{assert_framework, assert_source_file, assertion_lines, case, produce, rows, tree};

use AssertionKind::{Equality, Throws};
use AssertionStrength::Strong;

#[test]
fn a_call_of_an_asserting_helper_method_of_the_class_asserts() {
    let dir = tree(&[(
        "src/test/java/a/OptionTest.java",
        "package a;\n\
         \n\
         import static org.junit.jupiter.api.Assertions.assertEquals;\n\
         \n\
         import org.junit.jupiter.api.Test;\n\
         \n\
         class OptionTest {\n    \
         private static void checkOption(final Option option, final String opt) {\n        \
         assertEquals(opt, option.getOpt());\n    \
         }\n\
         \n    \
         private Option build(final String opt) {\n        \
         return Option.builder(opt).get();\n    \
         }\n\
         \n    \
         @Test\n    \
         void testBuilderMethods() {\n        \
         checkOption(Option.builder(\"a\").get(), \"a\");\n    \
         }\n\
         \n    \
         @Test\n    \
         void testThisHelper() {\n        \
         this.checkOption(build(\"b\"), \"b\");\n    \
         }\n\
         \n    \
         @Test\n    \
         void testBuildsOnly() {\n        \
         build(\"c\");\n    \
         }\n\
         }\n",
    )]);
    let frames: TestFrames = produce("java", dir.path());
    assert_eq!(
        rows(&assert_framework(
            &frames,
            "src/test/java/a/OptionTest.java",
            "junit5"
        )),
        [
            case("testBuilderMethods", &[(Equality, Strong)], Runs, &[], 16),
            case("testThisHelper", &[(Equality, Strong)], Runs, &[], 21),
            case("testBuildsOnly", &[], Runs, &[], 26),
        ]
    );
}

#[test]
fn a_try_whose_block_ends_in_fail_is_one_throws_assertion_spanning_its_handlers() {
    let file = assert_source_file(
        "java",
        "src/test/java/a/DecodeTest.java",
        "package a;\n\
         \n\
         import static org.junit.jupiter.api.Assertions.*;\n\
         import org.junit.jupiter.api.Test;\n\
         \n\
         class DecodeTest {\n    \
         @Test\n    \
         void testDecodeThrows() {\n        \
         try {\n            \
         decode(\"-%8en\");\n            \
         fail(\"Expecting exception: IllegalArgumentException\");\n        \
         } catch (IllegalArgumentException e) {\n            \
         assertEquals(\"a\", e.getMessage());\n        \
         }\n    \
         }\n\
         \n    \
         @Test\n    \
         void testDecodeCaught() {\n        \
         try {\n            \
         decode(\"x\");\n        \
         } catch (IllegalArgumentException e) {\n            \
         assertEquals(\"a\", e.getMessage());\n        \
         }\n    \
         }\n\
         \n    \
         @Test\n    \
         void testQualifiedFail() {\n        \
         try {\n            \
         decode(\"y\");\n            \
         Assertions.fail();\n        \
         } catch (IllegalStateException | IllegalArgumentException e) {\n        \
         }\n    \
         }\n\
         }\n",
        "junit5",
    );
    assert_eq!(
        assertion_lines(&file),
        [
            ("testDecodeThrows", Throws, Strong, 9, 14),
            ("testDecodeThrows", Equality, Strong, 13, 13),
            ("testDecodeCaught", Equality, Strong, 22, 22),
            ("testQualifiedFail", Throws, Strong, 28, 32),
        ]
    );
}
