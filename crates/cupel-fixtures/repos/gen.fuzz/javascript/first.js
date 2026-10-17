// The first character's code of text; the empty input throws.
function firstCode(text) {
  if (text.length === 0) {
    throw new Error("empty input");
  }
  return text.charCodeAt(0);
}

// The length of text, which never throws.
function size(text) {
  return text.length;
}

module.exports = { firstCode, size };
