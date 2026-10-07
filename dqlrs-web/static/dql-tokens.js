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
    "SELECT", "SCAN", "INSERT", "UPDATE", "CREATE", "ALTER",
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
  /* DROP and DELETE are destructive, so they render red rather than teal. */
  deleted: ["DROP", "DELETE"],
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

const WRITE_ACTIONS = new Set([
  "INSERT", "UPDATE", "DELETE", "DROP", "CREATE", "ALTER", "LOAD",
]);
const ACTION_PREFIX = new Set(["EXPLAIN", "ANALYZE"]);

/**
 * One entry per query. `band` is `write` (reddish), `alt` (very light stripe
 * on even queries), or `plain`. Offsets cover the query from its first code
 * character through the terminating semicolon.
 */
export function queryBands(text) {
  const src = String(text);
  const bands = [];
  let quote = null;
  let codeStart = -1;
  let action = "";

  function finish(end) {
    if (codeStart < 0) return;
    const write = WRITE_ACTIONS.has(action);
    bands.push({
      from: codeStart,
      to: end,
      write,
      index: bands.length,
      band: write ? "write" : bands.length % 2 === 0 ? "alt" : "plain",
    });
    codeStart = -1;
    action = "";
  }

  for (let i = 0; i < src.length; i += 1) {
    const ch = src[i];
    const nxt = src[i + 1] || "";
    if (quote) {
      if (ch === "\\") {
        i += 1;
        continue;
      }
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === "-" && nxt === "-") {
      while (i < src.length && src[i] !== "\n") i += 1;
      continue;
    }
    if (ch === "'" || ch === '"') {
      if (codeStart < 0) codeStart = i;
      quote = ch;
      continue;
    }
    if (/[A-Za-z_]/.test(ch)) {
      if (codeStart < 0) codeStart = i;
      let j = i + 1;
      while (j < src.length && /[A-Za-z0-9_.-]/.test(src[j])) j += 1;
      const word = src.slice(i, j).toUpperCase();
      if (!action || ACTION_PREFIX.has(action)) action = word;
      i = j - 1;
      continue;
    }
    if (ch === ";") {
      finish(i + 1);
      continue;
    }
    if (codeStart < 0 && !/\s/.test(ch)) codeStart = i;
  }
  finish(src.length);
  return bands;
}
