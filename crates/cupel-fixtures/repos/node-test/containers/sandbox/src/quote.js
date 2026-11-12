// ABOUTME: Shell quoting for the sandbox's commands: `shellQuote` quotes one word unless it is plain,
// ABOUTME: `joinCommand` joins quoted words; no test calls `joinCommand`, so coverage reports it.
function shellQuote(word) {
  if (/^[A-Za-z0-9_./-]+$/.test(word)) {
    return word;
  }
  return "'" + word.replace(/'/g, "'\\''") + "'";
}

function joinCommand(words) {
  return words.map(shellQuote).join(" ");
}

module.exports = { shellQuote, joinCommand };
