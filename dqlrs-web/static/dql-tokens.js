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
 * One entry per query. `band` is `write` (light red), `write-even` (a darker
 * red on even queries), `alt` (very light stripe on even reads), or `plain`.
 *
 * The colored range starts at a comment that touches the query and runs
 * through the query, a comment after it, and the blank lines that follow.
 * A blank line is the boundary and keeps the previous query's color.
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
      band: write
        ? bands.length % 2 === 0 ? "write-even" : "write"
        : bands.length % 2 === 0 ? "alt" : "plain",
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
  return expandQueryBands(src, bands);
}

/**
 * How one query should be painted.
 * `write` / `write-even` fill the line. `bar` / `bar-even` are gutter marks.
 * `alt` is the light even-read stripe. `plain` draws nothing.
 */
export function bandAppearance(band, options = {}) {
  const evenOdd = options.evenOdd !== false;
  const minimalWrite = Boolean(options.minimalWrite);
  if (band.write) {
    const even = evenOdd && band.index % 2 === 0;
    if (minimalWrite) return even ? "bar-even" : "bar";
    return even ? "write-even" : "write";
  }
  if (evenOdd && band.index % 2 === 0) return "alt";
  return "plain";
}

/**
 * The text Run will execute.
 * A selection that contains non-whitespace is returned as-is.
 * Otherwise the query band containing `head` is the current query.
 * Returns null when the cursor is outside every query.
 */
export function runTarget(text, head, anchor = head) {
  const src = String(text);
  const from = Math.max(0, Math.min(head, anchor, src.length));
  const to = Math.max(0, Math.min(Math.max(head, anchor), src.length));
  if (from < to && src.slice(from, to).trim()) {
    return { from, to, kind: "selection" };
  }
  const pos = Math.max(0, Math.min(head, src.length));
  for (const band of queryBands(src)) {
    if (pos >= band.from && (pos < band.to || (pos === band.to && band.to === src.length))) {
      return { from: band.from, to: band.to, kind: "query" };
    }
  }
  return null;
}

function expandQueryBands(src, bands) {
  if (!bands.length) return bands;
  const lines = src.split("\n");
  const starts = [];
  let cursor = 0;
  for (const line of lines) {
    starts.push(cursor);
    cursor += line.length + 1;
  }
  const lineIndexAt = (offset) => {
    let index = 0;
    for (let i = 0; i < starts.length; i += 1) {
      if (starts[i] <= offset) index = i;
      else break;
    }
    return index;
  };
  const isComment = (index) => lines[index].trim().startsWith("--");
  const isEmpty = (index) => lines[index].trim() === "";
  const claimed = new Array(lines.length).fill(false);
  const code = bands.map((band) => ({
    from: lineIndexAt(band.from),
    to: lineIndexAt(Math.max(band.from, band.to - 1)),
  }));
  for (const span of code) {
    for (let i = span.from; i <= span.to; i += 1) claimed[i] = true;
  }
  return bands.map((band, index) => {
    let start = code[index].from;
    let end = code[index].to;
    while (start > 0 && !claimed[start - 1] && isComment(start - 1)) {
      start -= 1;
      claimed[start] = true;
    }
    while (end + 1 < lines.length && !claimed[end + 1] && isComment(end + 1)) {
      end += 1;
      claimed[end] = true;
    }
    while (end + 1 < lines.length && !claimed[end + 1] && isEmpty(end + 1)) {
      end += 1;
      claimed[end] = true;
    }
    const to = end + 1 < lines.length ? starts[end + 1] : src.length;
    return { ...band, from: starts[start], to, lineFrom: start + 1, lineTo: end + 1 };
  });
}
