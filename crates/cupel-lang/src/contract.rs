// ABOUTME: The capture-name contract between each query file and the generic producer that reads it:
// ABOUTME: which capture names a query must, may, and may not use. See the crate docs for meanings.
use crate::spec::QueryFile;

/// Capture names, in one place, so producers and the contract test cannot disagree.
pub mod capture {
    // symbols.scm
    pub const SYMBOL_FUNCTION: &str = "symbol.function";
    pub const SYMBOL_METHOD: &str = "symbol.method";
    pub const SYMBOL_CLOSURE: &str = "symbol.closure";
    pub const SYMBOL_NAME: &str = "symbol.name";
    pub const SYMBOL_BODY: &str = "symbol.body";
    pub const SYMBOL_VISIBILITY: &str = "symbol.visibility";
    pub const VISIBILITY_PUBLIC: &str = "visibility.public";
    pub const VISIBILITY_INTERNAL: &str = "visibility.internal";
    pub const VISIBILITY_PRIVATE: &str = "visibility.private";
    pub const EXPORT_NAME: &str = "export.name";
    // Shared by symbols.scm and api.scm: namespace contributors.
    pub const SCOPE: &str = "scope";
    pub const SCOPE_NAME: &str = "scope.name";
    pub const NAMESPACE_NAME: &str = "namespace.name";
    // decisions.scm: `decision.<kind>`, kinds spelled as `DecisionKind` serializes.
    pub const DECISION_BRANCH: &str = "decision.branch";
    pub const DECISION_LOOP: &str = "decision.loop";
    pub const DECISION_BOOLEAN_OP: &str = "decision.boolean-op";
    pub const DECISION_CASE: &str = "decision.case";
    pub const DECISION_CATCH: &str = "decision.catch";
    pub const DECISION_EARLY_RETURN: &str = "decision.early-return";
    pub const DECISION_TERNARY: &str = "decision.ternary";
    pub const DECISION_PREFIX: &str = "decision.";
    // mutations.scm: `mut.<operator>`, operators spelled as `MutationOperator` serializes.
    pub const MUT_RELATIONAL: &str = "mut.relational";
    pub const MUT_EQUALITY: &str = "mut.equality";
    pub const MUT_ARITH_ADD_SUB: &str = "mut.arith-add-sub";
    pub const MUT_ARITH_MUL_DIV: &str = "mut.arith-mul-div";
    pub const MUT_BOOLEAN_OP: &str = "mut.boolean-op";
    pub const MUT_BOOLEAN_LITERAL: &str = "mut.boolean-literal";
    pub const MUT_ZERO_ONE: &str = "mut.zero-one";
    pub const MUT_STATEMENT_DELETION: &str = "mut.statement-deletion";
    pub const MUT_RETURN_VALUE: &str = "mut.return-value";
    pub const MUT_CONSTANT: &str = "mut.constant";
    pub const MUT_BOUNDARY: &str = "mut.boundary";
    pub const MUT_NEGATION: &str = "mut.negation";
    pub const MUT_PREFIX: &str = "mut.";
    // tests.scm
    pub const TEST_CASE: &str = "test.case";
    pub const TEST_NAME: &str = "test.name";
    pub const TEST_ASSERTION: &str = "test.assertion";
    pub const ASSERTION_NAME: &str = "assertion.name";
    pub const ASSERTION_IDIOM_PREFIX: &str = "assertion.idiom.";
    pub const TEST_MOCK: &str = "test.mock";
    pub const MOCK_TARGET: &str = "mock.target";
    pub const TEST_SKIP: &str = "test.skip";
    pub const SKIP_REASON: &str = "skip.reason";
    pub const TEST_SETUP: &str = "test.setup";
    pub const FRAMEWORK_PREFIX: &str = "framework.";
    // imports.scm
    pub const IMPORT: &str = "import";
    pub const IMPORT_SPECIFIER: &str = "import.specifier";
    pub const IMPORT_NAME: &str = "import.name";
    pub const IMPORT_MODULE: &str = "import.module";
    // api.scm: `api.<kind>`, kinds spelled as `ApiItemKind` serializes.
    pub const API_FUNCTION: &str = "api.function";
    pub const API_METHOD: &str = "api.method";
    pub const API_TYPE: &str = "api.type";
    pub const API_INTERFACE: &str = "api.interface";
    pub const API_CONSTANT: &str = "api.constant";
    pub const API_VARIABLE: &str = "api.variable";
    pub const API_MODULE: &str = "api.module";
    pub const API_PREFIX: &str = "api.";
    pub const API_NAME: &str = "api.name";
    pub const API_SIGNATURE: &str = "api.signature";
    pub const API_LOCAL: &str = "api.local";
    pub const ROOT_ENTRY_POINT: &str = "root.entry-point";
    pub const ROOT_HOOK: &str = "root.hook";
}

use capture as c;

/// What one query file's captures must look like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Contract {
    pub file: QueryFile,
    /// Each group needs at least one of its names among the query's captures.
    pub required: &'static [&'static [&'static str]],
    /// Further names the query may use.
    pub optional: &'static [&'static str],
    /// Prefixes under which any suffix is allowed, such as `framework.` in `@framework.vitest`.
    pub open: &'static [&'static str],
}

impl Contract {
    /// Whether `name` is a capture this file may use. Names starting with `_` are always
    /// allowed: they exist only for predicates such as `(#eq? @_macro "assert")`.
    pub fn allows(&self, name: &str) -> bool {
        name.starts_with('_')
            || self.required.iter().any(|group| group.contains(&name))
            || self.optional.contains(&name)
            || self
                .open
                .iter()
                .any(|prefix| name.len() > prefix.len() && name.starts_with(prefix))
    }

    /// Every way the capture `names` of a compiled query break this contract.
    pub fn check(&self, names: &[&str]) -> Result<(), Vec<String>> {
        let mut problems: Vec<String> = names
            .iter()
            .filter(|name| !self.allows(name))
            .map(|name| format!("@{name} is not a capture {} may use", self.file))
            .collect();
        for group in self.required {
            if !group.iter().any(|name| names.contains(name)) {
                let spelled: Vec<String> = group.iter().map(|name| format!("@{name}")).collect();
                problems.push(format!(
                    "{} must capture {}",
                    self.file,
                    spelled.join(" or ")
                ));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems)
        }
    }
}

const SYMBOLS: Contract = Contract {
    file: QueryFile::Symbols,
    required: &[
        &[c::SYMBOL_FUNCTION, c::SYMBOL_METHOD, c::SYMBOL_CLOSURE],
        &[c::SYMBOL_NAME],
    ],
    optional: &[
        c::SYMBOL_BODY,
        c::SYMBOL_VISIBILITY,
        c::VISIBILITY_PUBLIC,
        c::VISIBILITY_INTERNAL,
        c::VISIBILITY_PRIVATE,
        c::EXPORT_NAME,
        c::SCOPE,
        c::SCOPE_NAME,
        c::NAMESPACE_NAME,
    ],
    open: &[],
};

const DECISIONS: Contract = Contract {
    file: QueryFile::Decisions,
    required: &[&[
        c::DECISION_BRANCH,
        c::DECISION_LOOP,
        c::DECISION_BOOLEAN_OP,
        c::DECISION_CASE,
        c::DECISION_CATCH,
        c::DECISION_EARLY_RETURN,
        c::DECISION_TERNARY,
    ]],
    optional: &[],
    open: &[],
};

const MUTATIONS: Contract = Contract {
    file: QueryFile::Mutations,
    required: &[&[
        c::MUT_RELATIONAL,
        c::MUT_EQUALITY,
        c::MUT_ARITH_ADD_SUB,
        c::MUT_ARITH_MUL_DIV,
        c::MUT_BOOLEAN_OP,
        c::MUT_BOOLEAN_LITERAL,
        c::MUT_ZERO_ONE,
        c::MUT_STATEMENT_DELETION,
        c::MUT_RETURN_VALUE,
        c::MUT_CONSTANT,
        c::MUT_BOUNDARY,
        c::MUT_NEGATION,
    ]],
    optional: &[],
    open: &[],
};

const TESTS: Contract = Contract {
    file: QueryFile::Tests,
    required: &[&[c::TEST_CASE], &[c::TEST_NAME]],
    optional: &[
        c::TEST_ASSERTION,
        c::ASSERTION_NAME,
        c::TEST_MOCK,
        c::MOCK_TARGET,
        c::TEST_SKIP,
        c::SKIP_REASON,
        c::TEST_SETUP,
    ],
    open: &[c::ASSERTION_IDIOM_PREFIX, c::FRAMEWORK_PREFIX],
};

const IMPORTS: Contract = Contract {
    file: QueryFile::Imports,
    required: &[&[c::IMPORT], &[c::IMPORT_SPECIFIER]],
    optional: &[c::IMPORT_NAME, c::IMPORT_MODULE, c::SCOPE, c::SCOPE_NAME],
    open: &[],
};

const API: Contract = Contract {
    file: QueryFile::Api,
    required: &[
        &[
            c::API_FUNCTION,
            c::API_METHOD,
            c::API_TYPE,
            c::API_INTERFACE,
            c::API_CONSTANT,
            c::API_VARIABLE,
            c::API_MODULE,
        ],
        &[c::API_NAME],
    ],
    optional: &[
        c::API_SIGNATURE,
        c::API_LOCAL,
        c::ROOT_ENTRY_POINT,
        c::ROOT_HOOK,
        c::SCOPE,
        c::SCOPE_NAME,
        c::NAMESPACE_NAME,
    ],
    open: &[],
};

/// The contract for `file`.
pub fn contract(file: QueryFile) -> &'static Contract {
    match file {
        QueryFile::Symbols => &SYMBOLS,
        QueryFile::Decisions => &DECISIONS,
        QueryFile::Mutations => &MUTATIONS,
        QueryFile::Tests => &TESTS,
        QueryFile::Imports => &IMPORTS,
        QueryFile::Api => &API,
    }
}
