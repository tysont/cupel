// ABOUTME: The JavaScript and TypeScript harness (ADR-093 §6): `node` runs a driver that imports the entry's
// ABOUTME: file (TypeScript by Node's type stripping) and answers each input with the named export's result.
use super::{Call, Prepared, Problem, Site, write};
use cupel_core::Process;

/// Imports the file, then answers each input line, awaiting a promise.
const DRIVER: &str = r#"import { appendFileSync, readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const [file, name] = process.argv.slice(2);
const loaded = await import(pathToFileURL(file).href);
const fn = loaded[name] ?? loaded.default?.[name];
if (typeof fn !== "function") {
  throw new Error(`${file} exports no function ${name}`);
}

function show(value) {
  if (typeof value === "number" && !Number.isFinite(value)) return String(value);
  if (typeof value === "bigint") return `${value}n`;
  try {
    const text = JSON.stringify(value);
    return text === undefined ? String(value) : text;
  } catch {
    return String(value);
  }
}

async function answer(args) {
  try {
    return { returned: show(await fn(...args)) };
  } catch (error) {
    return { raised: error instanceof Error ? `${error.name}: ${error.message}` : String(error) };
  }
}

const out = process.env.CUPEL_DIFFERENTIAL_OUT;
appendFileSync(out, '{"ready":true}\n');
for (const line of readFileSync(process.env.CUPEL_DIFFERENTIAL_IN, "utf8").split("\n")) {
  if (line.trim()) appendFileSync(out, `${JSON.stringify(await answer(JSON.parse(line)))}\n`);
}
"#;

pub(super) fn prepare(call: &Call<'_>, site: &Site<'_>) -> Result<Prepared, Problem> {
    let driver = write(site, "driver.mjs", DRIVER)?;
    let process = Process::new("node", site.tree).args([
        driver.to_string_lossy().into_owned(),
        site.tree
            .join(&call.entry.file)
            .to_string_lossy()
            .into_owned(),
        call.entry.function.clone(),
    ]);
    Ok(Prepared::new(process, site.work.clone()))
}
