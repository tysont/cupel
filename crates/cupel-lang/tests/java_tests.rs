// ABOUTME: The Java TestFrames producer on hand-written trees: a JUnit test that calls an asserting helper method
// ABOUTME: of its own class asserts as that helper does, and a call of a helper that asserts nothing stays none.
mod support;

use cupel_core::syntax::{AssertionKind, AssertionStrength, TestFrames};
use support::Skip::Runs;
use support::{assert_framework, case, produce, rows, tree};

use AssertionKind::Equality;
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
