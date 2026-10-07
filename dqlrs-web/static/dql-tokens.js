/**
 * DQL token styles for the CodeMirror stream mode.
 *
 * Add a word to the matching group to highlight it. Groups map onto
 * CodeMirror tag names (`keyword`, `typeName`, `variableName.function`,
 * `bool`, `null`). The parser treats these words as case-insensitive.
 */
import { StringStream } from "./vendor/streamparser.js";

const groups = {
  keyword: [
    "SELECT", "SCAN", "INSERT", "UPDATE", "DELETE", "CREATE", "DROP", "ALTER",
    "DUMP", "LOAD", "EXPLAIN", "ANALYZE",
    "FROM", "WHERE", "INTO", "VALUES", "SET", "ADD", "REMOVE", "USING",
    "LIMIT", "ORDER", "BY", "ASC", "DESC", "AS", "KEYS", "IN", "CONSISTENT",
    "THROTTLE", "RETURNS", "SAVE",
    "TABLE", "INDEX", "GLOBAL", "HASH", "RANGE", "KEY", "THROUGHPUT", "TP",
    "IF", "NOT", "EXISTS", "INCLUDE", "ALL", "SCHEMA",
    "AND", "OR", "BETWEEN",
    "NONE", "UPDATED", "OLD", "NEW",
    "LS", "OPT", "OPTIONS", "HELP", "USE", "WATCH",
  ],
  typeName: ["STRING", "NUMBER", "BINARY", "BOOL", "BOOLEAN"],
  "variableName.function": [
    "COUNT", "SIZE", "BEGINS_WITH", "CONTAINS",
    "ATTRIBUTE_EXISTS", "ATTRIBUTE_NOT_EXISTS", "ATTRIBUTE_TYPE",
    "IF_NOT_EXISTS", "LIST_APPEND",
    "TIMESTAMP", "TS", "UTCTIMESTAMP", "UTCTS", "NOW", "UTCNOW", "MS", "INTERVAL",
  ],
  bool: ["TRUE", "FALSE"],
  null: ["NULL"],
};

const styleOf = new Map();
for (const [style, words] of Object.entries(groups)) {
  for (const word of words) {
    const key = word.toUpperCase();
    if (styleOf.has(key)) throw new Error(`duplicate DQL word ${key}`);
    styleOf.set(key, style);
  }
}

export function knownWords() {
  return [...styleOf.keys()];
}

export function styleForWord(word) {
  return styleOf.get(String(word).toUpperCase()) || null;
}

export function startState() {
  return { quote: null };
}

export function tokenDql(stream, state) {
  if (state.quote) {
    consumeString(stream, state);
    return "string";
  }
  if (stream.eatSpace()) return null;
  if (stream.match("--")) {
    stream.skipToEnd();
    return "comment";
  }
  if (stream.match(/^b["']/i) || stream.match(/^["']/)) {
    state.quote = stream.current().slice(-1);
    consumeString(stream, state);
    return "string";
  }
  if (stream.match(/^-?\d+(?:\.\d*)?/) || stream.match(/^\.\d+/)) return "number";
  if (stream.match(/^(?:<>|!=|<=|>=)/)) return "operator";
  if (stream.match(/^[*=<>!+\-/,;:()[\]{}]/)) return "operator";
  if (stream.match(/^[A-Za-z_][A-Za-z0-9_.-]*/)) {
    return styleForWord(stream.current()) || "variableName";
  }
  stream.next();
  return null;
}

function consumeString(stream, state) {
  let escaped = false;
  while (!stream.eol()) {
    const ch = stream.next();
    if (escaped) {
      escaped = false;
      continue;
    }
    if (ch === "\\") {
      escaped = true;
      continue;
    }
    if (ch === state.quote) {
      state.quote = null;
      return;
    }
  }
}

/** Tokenize source the same way the editor does. Whitespace tokens are omitted. */
export function highlightSource(text) {
  const state = startState();
  const tokens = [];
  const lines = String(text).split("\n");
  for (let i = 0; i < lines.length; i += 1) {
    const stream = new StringStream(lines[i], 4, 2, null);
    while (!stream.eol()) {
      const style = tokenDql(stream, state);
      const value = stream.current();
      stream.start = stream.pos;
      if (value && style) tokens.push({ text: value, style });
    }
    if (i < lines.length - 1 && state.quote) {
      /* a newline inside a string stays part of that string on the next line */
    }
  }
  return tokens;
}
