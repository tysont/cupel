// ABOUTME: The Go harness (ADR-093 §6): an internal test file laid over the entry's package with `-overlay`, so
// ABOUTME: nothing is written in the tree, built once with `go test -c`, decoding arguments into declared types.
use std::fs;

use cupel_core::Process;
use serde_json::json;

use super::{Call, Prepared, Problem, Site, build, write};

/// The test that answers each input line; `__PACKAGE__` and `__CALL__` are filled per entry.
/// Every name it adds to the package starts with `cupelDifferential`.
const TEMPLATE: &str = r#"package __PACKAGE__

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"testing"
)

func TestCupelDifferential(t *testing.T) {
	input, err := os.ReadFile(os.Getenv("CUPEL_DIFFERENTIAL_IN"))
	if err != nil {
		t.Fatal(err)
	}
	out, err := os.OpenFile(os.Getenv("CUPEL_DIFFERENTIAL_OUT"), os.O_APPEND|os.O_CREATE|os.O_WRONLY, 0o644)
	if err != nil {
		t.Fatal(err)
	}
	defer out.Close()
	cupelDifferentialWrite(out, map[string]any{"ready": true})
	for _, line := range bytes.Split(input, []byte("\n")) {
		if len(bytes.TrimSpace(line)) > 0 {
			cupelDifferentialWrite(out, cupelDifferentialAnswer(line))
		}
	}
}

func cupelDifferentialWrite(out *os.File, answer map[string]any) {
	text, _ := json.Marshal(answer)
	out.Write(append(text, '\n'))
}

func cupelDifferentialAnswer(line []byte) (answer map[string]any) {
	defer func() {
		if recovered := recover(); recovered != nil {
			answer = map[string]any{"raised": fmt.Sprint(recovered)}
		}
	}()
	var args []json.RawMessage
	cupelDifferentialDecode(line, &args)
	return map[string]any{"returned": cupelDifferentialShow(cupelDifferentialCall(args)...)}
}

func cupelDifferentialDecode(raw []byte, into any) {
	if err := json.Unmarshal(raw, into); err != nil {
		panic(err)
	}
}

func cupelDifferentialShow(results ...any) string {
	for at, result := range results {
		if err, ok := result.(error); ok {
			results[at] = err.Error()
		}
	}
	var value any = results
	if len(results) == 1 {
		value = results[0]
	}
	text, err := json.Marshal(value)
	if err != nil {
		return fmt.Sprintf("%#v", value)
	}
	return string(text)
}

func cupelDifferentialCall(args []json.RawMessage) []any {
__CALL__
}
"#;

/// The file name the harness takes in the package; nothing by this name is written there.
const TEST_FILE: &str = "cupel_differential_test.go";

pub(super) fn prepare(call: &Call<'_>, site: &Site<'_>) -> Result<Prepared, Problem> {
    let source = site.tree.join(&call.entry.file);
    let package_dir = source
        .parent()
        .ok_or_else(|| Problem::Skip("the entry's file has no directory".to_owned()))?;
    let contents = fs::read_to_string(&source)
        .map_err(|error| Problem::Skip(format!("cannot read `{}`: {error}", call.entry.file)))?;
    let package = package_name(&contents)
        .ok_or_else(|| Problem::Skip(format!("`{}` has no package clause", call.entry.file)))?;
    let harness = TEMPLATE
        .replace("__PACKAGE__", package)
        .replace("__CALL__", &call_body(call));
    let test = write(site, "harness_test.go", &harness)?;
    let overlay =
        json!({"Replace": {package_dir.join(TEST_FILE).to_string_lossy(): test.to_string_lossy()}});
    let overlay = write(site, "overlay.json", &overlay.to_string())?;
    let binary = site.work.join("harness.test");
    let paths = [&overlay, &binary].map(|path| path.to_string_lossy().into_owned());
    build(
        &Process::new("go", package_dir).args([
            "test", "-c", "-vet=off", "-overlay", &paths[0], "-o", &paths[1], ".",
        ]),
        site.cancel,
    )?;
    let process =
        Process::new(&paths[1], package_dir).args(["-test.run", "^TestCupelDifferential$"]);
    Ok(Prepared::new(process, site.work.clone()))
}

/// The name in a Go file's `package` clause.
fn package_name(text: &str) -> Option<&str> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("package "))
        .map(|rest| rest.split_whitespace().next().unwrap_or(rest))
}

/// Decodes each argument into its declared type, calls the function, and returns every result.
fn call_body(call: &Call<'_>) -> String {
    let decoded: Vec<String> = call
        .signature
        .params
        .iter()
        .enumerate()
        .map(|(at, param)| {
            format!(
                "\tvar a{at} {}\n\tcupelDifferentialDecode(args[{at}], &a{at})\n",
                param.text
            )
        })
        .collect();
    let args: Vec<String> = (0..call.signature.params.len())
        .map(|at| format!("a{at}"))
        .collect();
    let invocation = format!("{}({})", call.entry.function, args.join(", "));
    let results: Vec<String> = (0..call.signature.results)
        .map(|at| format!("r{at}"))
        .collect();
    let returned = if results.is_empty() {
        format!("\t{invocation}\n\treturn []any{{}}\n")
    } else {
        let results = results.join(", ");
        format!("\t{results} := {invocation}\n\treturn []any{{{results}}}\n")
    };
    decoded.concat() + &returned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::differential::entry::{Resolved, parse};
    use crate::differential::types;
    use cupel_core::syntax::SymbolKey;

    #[test]
    fn the_call_decodes_each_argument_and_assigns_every_result() {
        let entry = parse("ledger/ledger.go:Settle").unwrap();
        let resolved = Resolved {
            symbol: SymbolKey {
                file: entry.file.clone(),
                line: 1,
                col: 1,
            },
            namespace: "ledger".to_owned(),
            signature: "(_ []int64, _ int64) (int64, error)".to_owned(),
        };
        let signature = types::go(&resolved.signature).unwrap();
        let call = Call {
            entry: &entry,
            resolved: &resolved,
            signature: &signature,
        };
        assert_eq!(
            call_body(&call),
            "\tvar a0 []int64\n\tcupelDifferentialDecode(args[0], &a0)\n\
             \tvar a1 int64\n\tcupelDifferentialDecode(args[1], &a1)\n\
             \tr0, r1 := Settle(a0, a1)\n\treturn []any{r0, r1}\n"
        );
    }

    #[test]
    fn the_package_comes_from_the_package_clause() {
        assert_eq!(
            package_name("// Package ledger.\npackage ledger // money\n"),
            Some("ledger")
        );
        assert_eq!(package_name("func main() {}"), None);
    }
}
