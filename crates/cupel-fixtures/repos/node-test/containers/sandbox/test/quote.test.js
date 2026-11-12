// ABOUTME: The sandbox's node:test suite, run with `node --test`: `shellQuote` on a plain word and on a
// ABOUTME: word with a space.
const { describe, test } = require("node:test");
const assert = require("node:assert/strict");
const { shellQuote } = require("../src/quote");

describe("shellQuote", () => {
  test("leaves a plain word alone", () => {
    assert.equal(shellQuote("ls"), "ls");
  });

  test("quotes a word with a space", () => {
    assert.equal(shellQuote("a b"), "'a b'");
  });
});
