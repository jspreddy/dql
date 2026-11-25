// Table browser read picker: SCAN/SELECT from source, keys, filters, and order.

import { highlightParts } from "./dql-tokens.js";

export const COMPARE_OPS = [
  { id: "eq", label: "=", dql: "=" },
  { id: "ne", label: "≠", dql: "<>" },
  { id: "lt", label: "<", dql: "<" },
  { id: "le", label: "≤", dql: "<=" },
  { id: "gt", label: ">", dql: ">" },
  { id: "ge", label: "≥", dql: ">=" },
  { id: "between", label: "between", dql: "BETWEEN", values: 2 },
];

export const FUNCTION_OPS = [
  { id: "begins_with", label: "begins with", dql: "begins_with", fn: true },
  { id: "contains", label: "contains", dql: "contains", fn: true },
  { id: "exists", label: "exists", dql: "attribute_exists", fn: true, values: 0 },
  { id: "not_exists", label: "not exists", dql: "attribute_not_exists", fn: true, values: 0 },
  { id: "attribute_type", label: "type is", dql: "attribute_type", fn: true, type: true },
  { id: "size", label: "size", dql: "size", fn: true, size: true },
];

export const SIZE_COMPARE_OPS = COMPARE_OPS.filter((op) => op.id !== "between");

export const KEY_HASH_OPS = COMPARE_OPS.filter((op) => op.id === "eq");
export const KEY_RANGE_OPS = [
  ...COMPARE_OPS.filter((op) => op.id !== "ne"),
  FUNCTION_OPS.find((op) => op.id === "begins_with"),
];
export const KEY_SCAN_OPS = [
  ...COMPARE_OPS,
  FUNCTION_OPS.find((op) => op.id === "begins_with"),
];
export const FILTER_OPS = [...COMPARE_OPS, ...FUNCTION_OPS];

export const ATTR_TYPES = ["S", "N", "B", "BOOL", "NULL", "L", "M", "SS", "NS", "BS"];

const IDENT = /^[A-Za-z_][A-Za-z0-9_.-]*$/;

// The parser accepts any identifier, including words the highlighter paints as
// keywords. A name that is not an identifier cannot be quoted into a statement:
// the lexer treats quotes as strings and does not treat '' as an escape.
export function quoteIdent(name) {
  const text = String(name || "");
  return IDENT.test(text) ? text : "";
}

function unusableName(name) {
  const text = String(name || "").trim();
  if (!text || IDENT.test(text)) return "";
  return text;
}

export function quoteValue(raw, type) {
  const text = String(raw ?? "");
  const trimmed = text.trim();
  const kind = String(type || "").toUpperCase();
  if (kind === "NUMBER" || kind === "N") return trimmed;
  if ((kind === "BOOL" || kind === "BOOLEAN") && /^(true|false)$/i.test(trimmed)) {
    return trimmed.toUpperCase();
  }
  if (!kind) {
    if (/^-?\d+(?:\.\d+)?$/.test(trimmed)) return trimmed;
    if (/^(true|false|null)$/i.test(trimmed)) return trimmed.toUpperCase();
  }
  return "'" + text.replaceAll("'", "''") + "'";
}

const NUMBER_TEXT = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/;

/**
 * Why `raw` cannot be a value of key `name` with attribute `type`, or "" when it can. Only a
 * NUMBER key can reject a value: a string key quotes whatever is typed. Without this check a
 * number key passes the text into the statement unquoted, and the error only shows after Run.
 */
export function keyValueError(name, type, raw) {
  const kind = String(type || "").toUpperCase();
  const text = String(raw ?? "").trim();
  if ((kind === "NUMBER" || kind === "N") && text && !NUMBER_TEXT.test(text)) {
    return `${name} is a number. ${JSON.stringify(text)} is not.`;
  }
  return "";
}

export function parseBatchKeys(text) {
  const src = String(text || "");
  let i = 0;
  const keys = [];
  const errors = [];

  const isSep = (ch) => ch === "," || ch === "\n" || ch === "\r";
  const isSpace = (ch) => ch === " " || ch === "\t";

  function lineAt(index) {
    let line = 1;
    for (let pos = 0; pos < index && pos < src.length; pos += 1) {
      if (src[pos] === "\n") line += 1;
    }
    return line;
  }

  function skipIdle() {
    while (i < src.length && (isSpace(src[i]) || isSep(src[i]))) i += 1;
  }

  function skipSpace() {
    while (i < src.length && isSpace(src[i])) i += 1;
  }

  function parseQuoted(quote) {
    i += 1;
    let out = "";
    while (i < src.length) {
      const ch = src[i];
      if (ch === quote && src[i + 1] === quote) {
        out += quote;
        i += 2;
        continue;
      }
      if (ch === quote) {
        i += 1;
        return out;
      }
      out += ch;
      i += 1;
    }
    return null;
  }

  function parseBare() {
    const start = i;
    while (i < src.length) {
      const ch = src[i];
      if (isSep(ch) || ch === "(" || ch === ")") break;
      i += 1;
    }
    return src.slice(start, i).trim();
  }

  function parseValue() {
    skipSpace();
    if (i >= src.length) return { ok: true, value: "" };
    const ch = src[i];
    if (ch === "'" || ch === '"') {
      const start = i;
      const value = parseQuoted(ch);
      if (value === null) return { ok: false, error: `Line ${lineAt(start)} has an unclosed quote.` };
      return { ok: true, value };
    }
    if (isSep(ch) || ch === "(" || ch === ")") return { ok: true, value: "" };
    return { ok: true, value: parseBare() };
  }

  function collapseGroup(elems, openAt) {
    if (elems.every((elem) => elem.type === "value")) {
      // A tuple is a (hash, range) pair. Three or more plain values in one group is not a key.
      if (elems.length > 2) {
        errors.push(`Line ${lineAt(openAt)}: a key has at most two parts: (hash, range).`);
        return [];
      }
      if (elems.length === 2) return [[elems[0].value, elems[1].value]];
      return elems.map((elem) => [elem.value]);
    }
    const nested = [];
    for (const elem of elems) {
      if (elem.type === "value") nested.push([elem.value]);
      else nested.push(...elem.keys);
    }
    return nested;
  }

  function parseGroup(openAt) {
    const elems = [];
    while (i < src.length) {
      skipIdle();
      if (i >= src.length) {
        errors.push(`Line ${lineAt(openAt)} has an unclosed (.`);
        return collapseGroup(elems, openAt);
      }
      if (src[i] === ")") {
        i += 1;
        return collapseGroup(elems, openAt);
      }
      if (src[i] === "(") {
        const nestedAt = i;
        i += 1;
        elems.push({ type: "keys", keys: parseGroup(nestedAt) });
        continue;
      }
      const value = parseValue();
      if (!value.ok) {
        errors.push(value.error);
        return collapseGroup(elems, openAt);
      }
      if (value.value) elems.push({ type: "value", value: value.value });
    }
    errors.push(`Line ${lineAt(openAt)} has an unclosed (.`);
    return collapseGroup(elems, openAt);
  }

  skipIdle();
  while (i < src.length) {
    skipIdle();
    if (i >= src.length) break;
    const start = i;
    if (src[i] === ")") {
      errors.push(`Line ${lineAt(start)} has an extra ).`);
      i += 1;
      continue;
    }
    if (src[i] === "(") {
      i += 1;
      keys.push(...parseGroup(start));
      continue;
    }
    const value = parseValue();
    if (!value.ok) {
      errors.push(value.error);
      break;
    }
    if (value.value) keys.push([value.value]);
  }
  return { keys, errors };
}

/**
 * `keys` without repeats, and how many were dropped. Two keys are the same when they name the
 * same values (as DynamoDB sees them: 1 and 1.0 are one number). A pair whose hash value is also
 * listed alone is dropped too: the lone value already reads every item under that hash key.
 * DynamoDB rejects a batch get whose key list repeats a key, so this is more than tidying up.
 */
export function dedupeKeys(keys, table, extra, source) {
  const src = source || tableSource(table);
  const canonical = (value, name) => {
    const type = String(attributeType(table, extra, name) || "").toUpperCase();
    const text = String(value ?? "").trim();
    if ((type === "NUMBER" || type === "N") && text && Number.isFinite(Number(text))) return `N:${Number(text)}`;
    return `S:${String(value ?? "")}`;
  };
  const hashes = new Set(
    (src.rangeKey ? keys || [] : []).filter((fields) => fields.length === 1).map((fields) => canonical(fields[0], src.hashKey)),
  );
  const seen = new Set();
  const unique = [];
  for (const fields of keys || []) {
    const hash = canonical(fields[0], src.hashKey);
    if (fields.length > 1 && hashes.has(hash)) continue;
    const id = fields.map((value, index) => canonical(value, index === 0 ? src.hashKey : src.rangeKey)).join("\u0000");
    if (seen.has(id)) continue;
    seen.add(id);
    unique.push(fields);
  }
  return { keys: unique, dropped: (keys || []).length - unique.length };
}

/**
 * Pieces of the Keys box text for highlighting, covering every character in order:
 * `{ text, kind, key }`. `key` numbers each key (a lone value or a tuple) from 0, so keys can
 * alternate colors. `kind` is "hash" or "range" for values, "tuple" for a key's parentheses,
 * "punct" for commas and the parentheses around a list of tuples, "error" for what cannot be
 * a key (a tuple with three or more parts, an unclosed quote, an extra ")"), or "" for space.
 */
export function keyHighlightSpans(text) {
  const src = String(text || "");
  const tokens = [];
  let i = 0;
  while (i < src.length) {
    const ch = src[i];
    if (ch === "(" || ch === ")" || ch === ",") {
      tokens.push({ t: ch, s: i, e: i + 1 });
      i += 1;
    } else if (/\s/.test(ch)) {
      let j = i;
      while (j < src.length && /\s/.test(src[j])) j += 1;
      tokens.push({ t: "ws", s: i, e: j });
      i = j;
    } else if (ch === "'" || ch === '"') {
      let j = i + 1;
      let closed = false;
      while (j < src.length) {
        if (src[j] === ch && src[j + 1] === ch) {
          j += 2;
          continue;
        }
        j += 1;
        if (src[j - 1] === ch) {
          closed = true;
          break;
        }
      }
      tokens.push({ t: "v", s: i, e: j, bad: !closed });
      i = j;
    } else {
      let j = i;
      while (j < src.length && !"(),\n\r".includes(src[j])) j += 1;
      let end = j;
      while (end > i && /\s/.test(src[end - 1])) end -= 1;
      tokens.push({ t: "v", s: i, e: end });
      if (end < j) tokens.push({ t: "ws", s: end, e: j });
      i = j;
    }
  }

  // Group tokens by parentheses: a group holds values, nested groups, and separators.
  let at = 0;
  const readGroup = (open) => {
    const node = { open, close: null, items: [] };
    while (at < tokens.length) {
      const token = tokens[at];
      at += 1;
      if (token.t === "(") node.items.push(readGroup(token));
      else if (token.t === ")") {
        if (open) {
          node.close = token;
          return node;
        }
        node.items.push({ stray: token });
      } else node.items.push(token);
    }
    return node;
  };
  const root = readGroup(null);

  const kinds = new Map();
  const keyOf = new Map();
  let keyIndex = 0;
  const mark = (token, kind, key = null) => {
    if (!token) return;
    kinds.set(token, kind);
    if (key !== null) keyOf.set(token, key);
  };
  const isGroup = (item) => item && item.items;
  const visitList = (node) => {
    for (const item of node.items) {
      if (item.stray) mark(item.stray, "error");
      else if (isGroup(item)) visitGroup(item);
      else if (item.t === "v") {
        mark(item, item.bad ? "error" : "hash", keyIndex);
        keyIndex += 1;
      } else if (item.t === ",") mark(item, "punct");
    }
  };
  const visitGroup = (group) => {
    if (group.items.some(isGroup)) {
      // A list of tuples: its own parentheses are punctuation.
      mark(group.open, "punct");
      mark(group.close, group.close ? "punct" : null);
      visitList(group);
      return;
    }
    const values = group.items.filter((item) => item.t === "v");
    const key = keyIndex;
    keyIndex += 1;
    const tooMany = values.length > 2 || !group.close;
    mark(group.open, tooMany ? "error" : "tuple", key);
    mark(group.close, tooMany ? "error" : "tuple", key);
    values.forEach((value, index) => {
      mark(value, tooMany || value.bad ? "error" : index === 0 ? "hash" : "range", key);
    });
    for (const item of group.items) {
      if (item.t === "," || item.t === "ws") mark(item, item.t === "," ? "punct" : "", key);
    }
  };
  visitList(root);

  return tokens.map((token) => ({
    text: src.slice(token.s, token.e),
    kind: kinds.get(token) || "",
    key: keyOf.has(token) ? keyOf.get(token) : null,
  }));
}

export function formatKeysInClause(keys, table, extra, source) {
  const src = source || tableSource(table);
  return (keys || []).map((fields) => {
    const quoted = fields.map((value, index) => {
      const name = index === 0 ? src.hashKey : src.rangeKey;
      return quoteValue(value, attributeType(table, extra, name));
    });
    return quoted.length === 1 ? quoted[0] : `(${quoted.join(", ")})`;
  }).join(", ");
}

export function formatIndexKeySelect(tableName, source, fields, table, extra) {
  const hash = source && source.hashKey ? source.hashKey : "id";
  const hashVal = quoteValue(fields[0], attributeType(table, extra, hash));
  let sql = `SELECT * FROM ${quoteIdent(tableName)} WHERE ${quoteIdent(hash)} = ${hashVal}`;
  if (fields.length > 1 && source && source.rangeKey) {
    sql += ` AND ${quoteIdent(source.rangeKey)} = ${quoteValue(fields[1], attributeType(table, extra, source.rangeKey))}`;
  }
  if (usesIndex(source)) sql += usingClause(source);
  return sql;
}

export function batchKeyHint(table, sourceName) {
  const source = sourceOf(table, sourceName, "query");
  const hash = source.hashKey || "hash";
  if (source.rangeKey) {
    return `${hash} keys, or (${hash}, ${source.rangeKey}) pairs`;
  }
  return `${hash} keys: a, b, c`;
}

export function keysModeError(table, parsed, sourceName, extra) {
  const result = parsed || parseBatchKeys("");
  if (result.errors.length) return result.errors[0];
  if (!result.keys.length) return "Keys needs at least one key.";
  const source = sourceOf(table, sourceName, "query");
  const tablePk = !usesIndex(source);
  const hasRange = Boolean(source.rangeKey);
  if (!hasRange && result.keys.some((key) => key.length > 1)) {
    return tablePk ? "This table only has a hash key." : "This index only has a hash key.";
  }
  for (const fields of result.keys) {
    for (const [index, value] of fields.entries()) {
      const name = index === 0 ? source.hashKey : source.rangeKey;
      if (!name) continue;
      const message = keyValueError(name, attributeType(table, extra, name), value);
      if (message) return message;
    }
  }
  return "";
}

export function opById(id, ops) {
  return (ops || FILTER_OPS).find((op) => op.id === id) || COMPARE_OPS[0];
}

export function opsFor(kind, role, mode) {
  if (kind === "filter") return FILTER_OPS;
  if (mode === "query" && role === "hash") return KEY_HASH_OPS;
  if (mode === "query") return KEY_RANGE_OPS;
  return KEY_SCAN_OPS;
}

/**
 * The source menu's values: "" is "--" (no USING; dqlrs picks an index itself), TABLE_SOURCE
 * ("-") is the table (`USING -` turns that off), and anything else names an index (`USING name`).
 * Keys mode reads by primary key, where the table and "--" are the same read (KEYS IN).
 */
export const TABLE_SOURCE = "-";

/** True when `source` names an index rather than "--" or the table. */
export function usesIndex(source) {
  return Boolean(source && source.name && source.name !== TABLE_SOURCE);
}

/** ` USING …` for `source`, or "" for "--". */
export function usingClause(source) {
  if (!source || !source.name) return "";
  if (source.name === TABLE_SOURCE) return " USING " + TABLE_SOURCE;
  const name = quoteIdent(source.name);
  if (!name) return "";
  return " USING " + name;
}

export function tableSource(table) {
  const keys = Array.isArray(table && table.keys) ? table.keys : [];
  const hash = keys.find((key) => key.role === "hash");
  const range = keys.find((key) => key.role === "range");
  return {
    name: "",
    kind: "table",
    hashKey: hash ? hash.name : "",
    rangeKey: range ? range.name : "",
  };
}

export function listedIndexes(table) {
  return Array.isArray(table && table.indexes) ? table.indexes : [];
}

export function availableSources(table, mode) {
  const sources = [tableSource(table), { ...tableSource(table), name: TABLE_SOURCE }];
  for (const index of listedIndexes(table)) {
    if (mode === "scan" && index.kind === "local") continue;
    sources.push({
      name: index.name,
      kind: index.kind || "global",
      hashKey: index.hashKey || "",
      rangeKey: index.rangeKey || "",
    });
  }
  return sources;
}

export function sourceOf(table, sourceName, mode) {
  const sources = availableSources(table, mode);
  return sources.find((source) => source.name === (sourceName || "")) || sources[0];
}

export function sourceKeys(table, sourceName, mode) {
  const source = sourceOf(table, sourceName, mode);
  const keys = [];
  if (source.hashKey) keys.push({ name: source.hashKey, role: "hash" });
  if (source.rangeKey && source.rangeKey !== source.hashKey) {
    keys.push({ name: source.rangeKey, role: "range" });
  }
  return keys;
}

export function attributeType(table, extra, name) {
  const listed = Array.isArray(table && table.attributes) ? table.attributes : [];
  const hit = listed.find((attr) => attr.name === name);
  if (hit && hit.type) return hit.type;
  const extras = extra instanceof Map ? extra : null;
  return extras && extras.get(name) || "";
}

export function roleLabel(role) {
  if (role === "hash") return "hash";
  if (role === "range") return "range";
  if (role === "index-hash") return "index hash";
  if (role === "index-range") return "index range";
  return "";
}

export function attributeLabel(item) {
  const role = roleLabel(item && item.role);
  const name = item && item.name ? item.name : "";
  return role ? `${name} (${role})` : name;
}

export function filterAttributes(table, extraNames) {
  const names = [];
  const seen = new Set();
  const add = (name, type, role) => {
    if (!name || seen.has(name)) return;
    seen.add(name);
    names.push({ name, type: type || "", role: role || "" });
  };
  for (const key of sourceKeys(table, "", "query")) {
    add(key.name, attributeType(table, extraNames, key.name), key.role);
  }
  for (const index of listedIndexes(table)) {
    add(index.hashKey, attributeType(table, extraNames, index.hashKey), "index-hash");
    add(index.rangeKey, attributeType(table, extraNames, index.rangeKey), "index-range");
  }
  const listed = Array.isArray(table && table.attributes) ? table.attributes : [];
  for (const attr of listed) add(attr.name, attr.type);
  const extras = extraNames instanceof Map ? extraNames.keys() : extraNames || [];
  for (const name of extras) add(name, extraNames instanceof Map ? extraNames.get(name) : "");
  return names;
}

export function keyAttributeSuggestions(table, sourceName, mode, extraNames) {
  const names = [];
  const seen = new Set();
  const add = (name, role) => {
    if (!name || seen.has(name)) return;
    seen.add(name);
    names.push({
      name,
      type: attributeType(table, extraNames, name),
      role: role || "",
    });
  };
  for (const key of sourceKeys(table, sourceName, mode)) add(key.name, key.role);
  for (const attr of filterAttributes(table, extraNames)) {
    add(attr.name, attr.role);
  }
  return names;
}

export function formatCondition(row, table, extra) {
  if (!row || !row.attribute) return "";
  const ops = opsFor(row.kind || "filter", row.role, row.mode);
  const op = opById(row.op, ops);
  const field = quoteIdent(row.attribute);
  if (!field) return "";
  const type = attributeType(table, extra, row.attribute);
  const values = op.values === 0 ? 0 : op.values === 2 ? 2 : 1;
  if (values === 1 && !String(row.value ?? "").length && !op.type && !op.size) return "";
  if (op.size && !String(row.value ?? "").length) return "";
  if (values === 2 && (!String(row.value ?? "").length || !String(row.value2 ?? "").length)) return "";
  if (op.size) {
    const cmp = SIZE_COMPARE_OPS.find((item) => item.id === (row.sizeOp || "gt")) || SIZE_COMPARE_OPS.find((item) => item.id === "gt");
    return `size(${field}) ${cmp.dql} ${quoteValue(row.value, "NUMBER")}`;
  }
  if (op.fn && values === 0) return `${op.dql}(${field})`;
  if (op.fn && op.type) {
    const ty = String(row.value || "S").trim() || "S";
    return `${op.dql}(${field}, '${ty.replaceAll("'", "''")}')`;
  }
  if (op.fn) return `${op.dql}(${field}, ${quoteValue(row.value, type)})`;
  if (op.id === "between") {
    return `${field} BETWEEN ${quoteValue(row.value, type)} AND ${quoteValue(row.value2, type)}`;
  }
  return `${field} ${op.dql} ${quoteValue(row.value, type)}`;
}

export function defaultReadState() {
  return {
    source: "",
    mode: "scan",
    order: "forward",
    keys: [],
    filters: [],
    batchKeys: "",
  };
}

export function ensureQueryKeys(table, state) {
  const next = { ...state, keys: (state.keys || []).map((row) => ({ ...row })) };
  if (next.mode !== "query") {
    for (const row of next.keys) row.locked = false;
    return next;
  }
  const keys = sourceKeys(table, next.source, next.mode);
  const hash = keys.find((key) => key.role === "hash");
  if (!hash) {
    if (!next.keys.length) {
      next.keys = [
        { id: "hash", attribute: "", op: "eq", value: "", value2: "", role: "hash", locked: false },
      ];
    }
    return next;
  }
  const existing = next.keys.find((row) => row.attribute === hash.name);
  if (existing) {
    existing.role = "hash";
    existing.op = "eq";
    existing.locked = true;
    return next;
  }
  next.keys = [
    { id: "hash", attribute: hash.name, op: "eq", value: "", value2: "", role: "hash", locked: true },
    ...next.keys,
  ];
  return next;
}

// Every name the statement will contain, with what it is. The statement and readError use the
// same list, so a name that cannot be written is always reported, and a name the statement does
// not use (a key attribute in a plain Scan) never blocks it.
function statementNames(tableName, table, current, source) {
  const names = [[tableName || (table && table.name) || "", "table"]];
  for (const row of current.keys || []) names.push([row && row.attribute, "key"]);
  for (const row of current.filters || []) names.push([row && row.attribute, "filter"]);
  if (source && source.name && source.name !== TABLE_SOURCE) names.push([source.name, "index"]);
  if (source && current.mode === "keys" && usesIndex(source)) {
    // An index key read is a SELECT ... WHERE on the index keys.
    names.push([source.hashKey, "key"], [source.rangeKey, "key"]);
  }
  if (source && current.mode === "scan" && current.order === "reverse") {
    names.push([source.rangeKey || source.hashKey, "key"]);
  }
  return names.filter(([name]) => String(name || "").trim());
}

function unusableStatementName(tableName, table, current) {
  const source = sourceOf(table, current.source, current.mode === "keys" ? "query" : current.mode);
  for (const [value, kind] of statementNames(tableName, table, current, source)) {
    const bad = unusableName(value);
    if (bad) return `${bad} cannot be used as a ${kind} name`;
  }
  return "";
}

export function buildReadStatement(tableName, table, state, extra) {
  const name = tableName || (table && table.name) || "";
  if (!name || !IDENT.test(name)) return "";
  const current = ensureQueryKeys(table, state || defaultReadState());
  if (unusableStatementName(name, table, current)) return "";
  if (current.mode === "keys") {
    const parsed = parseBatchKeys(current.batchKeys);
    if (keysModeError(table, parsed, current.source, extra)) return "";
    const source = sourceOf(table, current.source, "query");
    const { keys } = dedupeKeys(parsed.keys, table, extra, source);
    if (usesIndex(source)) {
      return keys
        .map((fields) => formatIndexKeySelect(name, source, fields, table, extra))
        .join("; ");
    }
    // One KEYS IN for the table. dqlrs reads whole keys with a batch get and, on a table with a
    // range key, every item under a lone hash value.
    return `SELECT * FROM ${quoteIdent(name)} KEYS IN ${formatKeysInClause(keys, table, extra, source)}`;
  }
  const source = sourceOf(table, current.source, current.mode);
  const verb = current.mode === "query" ? "SELECT" : "SCAN";
  const keys = (current.keys || []).map((row) => formatCondition({
    ...row,
    kind: "key",
    mode: current.mode,
    role: row.role || sourceKeys(table, current.source, current.mode).find((key) => key.name === row.attribute)?.role,
  }, table, extra)).filter(Boolean);
  const filters = (current.filters || []).map((row) => formatCondition({
    ...row,
    kind: "filter",
    mode: current.mode,
  }, table, extra)).filter(Boolean);
  const where = [...keys, ...filters];
  let sql = `${verb} * FROM ${quoteIdent(name)}`;
  if (where.length) sql += " WHERE " + where.join(" AND ");
  sql += usingClause(source);
  if (current.order === "reverse") {
    if (current.mode === "query") sql += " DESC";
    else {
      const field = source.rangeKey || source.hashKey;
      if (field) sql += " ORDER BY " + quoteIdent(field) + " DESC";
    }
  }
  return sql;
}

export function readError(table, state, extra) {
  const current = ensureQueryKeys(table, state || defaultReadState());
  const unusable = unusableStatementName((table && table.name) || "", table, current);
  if (unusable) return unusable;
  if (current.mode === "keys") {
    return keysModeError(table, parseBatchKeys(current.batchKeys), current.source, extra);
  }
  if (current.mode === "query") {
    const hash = sourceKeys(table, current.source, current.mode).find((key) => key.role === "hash");
    const row = hash
      ? (current.keys || []).find((item) => item.attribute === hash.name)
      : (current.keys || []).find((item) => String(item.attribute || "").trim());
    if (!row || !String(row.attribute || "").trim()) return "Query needs a hash key.";
    if (!String(row.value ?? "").length) return "Query needs a hash key value.";
  }
  return keyRowsError(table, current, extra);
}

// The key rows of Query and Scan compare a typed key with what was typed. A NUMBER key must get
// a number, the same check Keys mode makes.
function keyRowsError(table, current, extra) {
  for (const row of current.keys || []) {
    const name = String(row.attribute || "").trim();
    if (!name) continue;
    const op = opById(row.op, opsFor("key", row.role, current.mode));
    if (op.fn || op.size || op.type || op.values === 0) continue;
    const type = attributeType(table, extra, name);
    const values = op.values === 2 ? [row.value, row.value2] : [row.value];
    for (const value of values) {
      const message = keyValueError(name, type, value);
      if (message) return message;
    }
  }
  return "";
}

function option(select, value, label, selected) {
  const node = document.createElement("option");
  node.value = value;
  node.textContent = label;
  if (selected) node.selected = true;
  select.append(node);
}

function fillSelect(select, items, selected) {
  const wanted = selected || "";
  select.replaceChildren();
  for (const item of items) {
    option(select, item.value, item.label, item.value === wanted);
  }
  if (wanted && ![...select.options].some((node) => node.value === wanted)) {
    option(select, wanted, wanted, true);
  }
  select.value = wanted;
}

function fillDatalist(list, items, current) {
  list.replaceChildren();
  const seen = new Set();
  for (const item of items || []) {
    if (!item || !item.name || seen.has(item.name)) continue;
    seen.add(item.name);
    const node = document.createElement("option");
    node.value = item.name;
    const label = attributeLabel(item);
    if (label && label !== item.name) node.label = label;
    list.append(node);
  }
  if (current && !seen.has(current)) {
    const node = document.createElement("option");
    node.value = current;
    list.append(node);
  }
}

function paintPreview(node, text) {
  node.replaceChildren();
  if (!text) return;
  for (const part of highlightParts(text)) {
    if (!part.style) {
      node.append(part.text);
      continue;
    }
    const span = document.createElement("span");
    span.className = "tok-" + part.style.replaceAll(".", "-");
    span.textContent = part.text;
    node.append(span);
  }
}

function newRow(attribute, op, extra) {
  return {
    id: "r" + Math.random().toString(36).slice(2, 9),
    attribute: attribute || "",
    op: op || "eq",
    value: "",
    value2: "",
    role: "",
    locked: false,
    ...extra,
  };
}

// The Keys box interface keys-editor.js also provides, over a plain textarea.
function textareaKeysEditor(textarea) {
  return {
    get value() {
      return textarea.value;
    },
    set value(text) {
      textarea.value = text;
    },
    set placeholder(text) {
      textarea.placeholder = text;
    },
    focus: () => textarea.focus(),
    hasFocus: () => document.activeElement === textarea,
    onInput: (listener) => textarea.addEventListener("input", listener),
  };
}

/**
 * The read the browser sends: what the picker holds, for `POST /api/table-read`, `/api/rows`,
 * and `/api/explain`. The server looks up the table's definition itself.
 */
export function readRequest(table, state, extra) {
  const current = ensureQueryKeys(table, state || defaultReadState());
  const strip = (row) => ({
    attribute: row.attribute || "",
    op: row.op || "eq",
    value: String(row.value ?? ""),
    value2: String(row.value2 ?? ""),
    role: row.role || "",
    sizeOp: row.sizeOp || "",
  });
  return {
    table: (table && table.name) || "",
    mode: current.mode,
    source: current.source || "",
    order: current.order || "forward",
    keys: (current.keys || []).map(strip),
    filters: (current.filters || []).map(strip),
    batchKeys: current.batchKeys || "",
    extra: extra instanceof Map ? [...extra.keys()] : [...(extra || [])],
  };
}

// How long the preview waits after the last keystroke before it asks the server.
const PREVIEW_DELAY_MS = 120;

export function attachTableQuery(host, handlers = {}) {
  const onRun = handlers.onRun || (() => {});
  const onChange = handlers.onChange || (() => {});
  // `preview(read)` resolves to `{ dql, error }` from the server. Without it (unit tests), the
  // local builder answers.
  const previewRead = handlers.preview || null;
  let table = null;
  let extra = new Map();
  let state = defaultReadState();
  let ran = "";
  let focusAttr = "";

  const sourceSelect = host.querySelector("#table-query-source");
  const scanBtn = host.querySelector("#table-query-scan");
  const queryBtn = host.querySelector("#table-query-query");
  const keysModeBtn = host.querySelector("#table-query-keys-mode");
  const orderGroup = host.querySelector("#table-query-order");
  const forwardBtn = host.querySelector("#table-query-forward");
  const reverseBtn = host.querySelector("#table-query-reverse");
  const runBtn = host.querySelector("#table-query-run");
  const addKey = host.querySelector("#table-query-add-key");
  const addFilter = host.querySelector("#table-query-add-filter");
  const keySection = host.querySelector("#table-query-key-section");
  const filterSection = host.querySelector("#table-query-filter-section");
  const keyHost = host.querySelector("#table-query-keys");
  const filterHost = host.querySelector("#table-query-filters");
  const batchSection = host.querySelector("#table-query-batch");
  // The Keys box. app.js passes a CodeMirror editor (keys-editor.js) through
  // handlers.createKeysEditor; without one, the plain textarea stands in.
  const batchTextarea = host.querySelector("#table-query-batch-keys");
  const batchInput = handlers.createKeysEditor
    ? handlers.createKeysEditor(batchTextarea, { submit: () => host.querySelector("#table-query-run")?.click() })
    : textareaKeysEditor(batchTextarea);
  const batchHint = host.querySelector("#table-query-batch-hint");
  const preview = host.querySelector("#table-query-dql");
  const errorNode = host.querySelector("#table-query-error");

  function currentSource() {
    return sourceOf(table, state.source, state.mode);
  }

  function statement() {
    return buildReadStatement(table && table.name, table, state, extra);
  }

  function error() {
    return readError(table, state, extra);
  }

  function unusedKeys() {
    const used = new Set((state.keys || []).map((row) => row.attribute));
    return sourceKeys(table, state.source, state.mode).filter((key) => !used.has(key.name));
  }

  function syncModeButtons() {
    const isKeys = state.mode === "keys";
    scanBtn.setAttribute("aria-pressed", state.mode === "scan" ? "true" : "false");
    queryBtn.setAttribute("aria-pressed", state.mode === "query" ? "true" : "false");
    keysModeBtn.setAttribute("aria-pressed", isKeys ? "true" : "false");
    forwardBtn.setAttribute("aria-pressed", state.order === "forward" ? "true" : "false");
    reverseBtn.setAttribute("aria-pressed", state.order === "reverse" ? "true" : "false");
    orderGroup.classList.toggle("hidden", isKeys);
    keySection.classList.toggle("hidden", isKeys);
    filterSection.classList.toggle("hidden", isKeys);
    batchSection.classList.toggle("hidden", !isKeys);
    const source = sourceOf(table, state.source, "query");
    const hash = source.hashKey || "hash";
    const range = source.rangeKey;
    paintBatchHint();
    batchInput.placeholder = range ? `(${hash}, ${range})` : "a, b, c";
    if (!batchInput.hasFocus()) batchInput.value = state.batchKeys || "";
  }

  function sourceItems(mode, tableLabel) {
    return availableSources(table, mode).map((source) => {
      if (!source.name) return { value: "", label: "--" };
      if (source.name === TABLE_SOURCE) return { value: TABLE_SOURCE, label: tableLabel };
      const keys = [source.hashKey, source.rangeKey].filter(Boolean).join(", ");
      const kind = source.kind === "local" ? "local" : "global";
      return { value: source.name, label: keys ? `${source.name} (${kind}: ${keys})` : `${source.name} (${kind})` };
    });
  }

  function paintSources() {
    const selected = state.source || "";
    const sourceMode = state.mode === "keys" ? "query" : state.mode;
    fillSelect(sourceSelect, sourceItems(sourceMode, "Table"), selected);
    if (![...sourceSelect.options].some((node) => node.value === selected)) {
      state.source = "";
      sourceSelect.value = "";
    }
  }

  function suggestionsFor(kind) {
    if (kind === "key") return keyAttributeSuggestions(table, state.source, state.mode, extra);
    return filterAttributes(table, extra);
  }

  function paintConditionRow(kind, row) {
    const wrap = document.createElement("div");
    wrap.className = "table-query-cond";
    wrap.dataset.kind = kind;
    wrap.dataset.id = row.id;
    wrap.dataset.testid = kind === "key" ? "table-query-key-row" : "table-query-filter-row";

    const suggestions = suggestionsFor(kind);
    const role = row.role || (suggestions.find((item) => item.name === row.attribute) || {}).role || "";
    const ops = opsFor(kind, role, state.mode);
    const list = document.createElement("datalist");
    list.id = `table-query-attr-list-${kind}-${row.id}`;
    fillDatalist(list, suggestions, row.attribute);
    const attr = document.createElement("input");
    attr.type = "text";
    attr.dataset.field = "attribute";
    attr.setAttribute("aria-label", kind === "key" ? "Key attribute" : "Filter attribute");
    attr.dataset.testid = "table-query-attr";
    attr.setAttribute("list", list.id);
    attr.spellcheck = false;
    attr.autocomplete = "off";
    attr.placeholder = "attribute";
    attr.value = row.attribute || "";
    if (row.locked) attr.readOnly = true;

    const op = document.createElement("select");
    op.dataset.field = "op";
    op.setAttribute("aria-label", "Condition");
    op.dataset.testid = "table-query-op";
    fillSelect(op, ops.map((item) => ({ value: item.id, label: item.label })), row.op);
    if (row.locked && state.mode === "query" && role === "hash") op.disabled = true;

    const value = document.createElement("input");
    value.type = "text";
    value.dataset.field = "value";
    value.spellcheck = false;
    value.autocomplete = "off";
    value.dataset.testid = "table-query-value";
    value.setAttribute("aria-label", "Value");
    value.placeholder = "value";
    value.value = row.value || "";

    const value2 = document.createElement("input");
    value2.type = "text";
    value2.dataset.field = "value2";
    value2.spellcheck = false;
    value2.autocomplete = "off";
    value2.dataset.testid = "table-query-value2";
    value2.setAttribute("aria-label", "Upper value");
    value2.placeholder = "and";
    value2.value = row.value2 || "";

    const sizeOp = document.createElement("select");
    sizeOp.dataset.field = "sizeOp";
    sizeOp.setAttribute("aria-label", "Size comparison");
    sizeOp.dataset.testid = "table-query-size-op";
    fillSelect(sizeOp, SIZE_COMPARE_OPS.map((item) => ({ value: item.id, label: item.label })), row.sizeOp || "gt");

    const type = document.createElement("select");
    type.dataset.field = "type";
    type.setAttribute("aria-label", "Attribute type");
    type.dataset.testid = "table-query-type";
    fillSelect(type, ATTR_TYPES.map((item) => ({ value: item, label: item })), row.value || "S");

    const remove = document.createElement("button");
    remove.type = "button";
    remove.dataset.field = "remove";
    remove.dataset.testid = "table-query-remove";
    remove.setAttribute("aria-label", "Remove condition");
    remove.textContent = "×";
    if (row.locked) remove.hidden = true;

    wrap.append(attr, list, op, sizeOp, value, value2, type, remove);
    syncValueFields(wrap, opById(row.op, ops));
    return wrap;
  }

  function refreshAttributeLists() {
    for (const node of host.querySelectorAll(".table-query-cond")) {
      const list = node.querySelector("datalist");
      const input = node.querySelector("[data-field=attribute]");
      if (!list || !input) continue;
      fillDatalist(list, suggestionsFor(node.dataset.kind), input.value);
    }
  }

  function syncValueFields(wrap, op) {
    const values = op.values === 0 ? 0 : op.values === 2 ? 2 : 1;
    wrap.querySelector("[data-field=sizeOp]").hidden = !op.size;
    wrap.querySelector("[data-field=value]").hidden = op.type || values === 0;
    wrap.querySelector("[data-field=value2]").hidden = values !== 2;
    wrap.querySelector("[data-field=type]").hidden = !op.type;
  }

  function paintRows() {
    keyHost.replaceChildren();
    for (const row of state.keys) {
      const node = paintConditionRow("key", row);
      bindRowEvents("key", node);
      keyHost.append(node);
    }
    filterHost.replaceChildren();
    for (const row of state.filters) {
      const node = paintConditionRow("filter", row);
      bindRowEvents("filter", node);
      filterHost.append(node);
    }
    addKey.disabled = false;
    if (focusAttr) {
      const node = host.querySelector(`[data-id="${focusAttr}"] [data-field=attribute]`);
      node?.focus();
      focusAttr = "";
    }
  }

  function paint() {
    const open = Boolean(table && table.name);
    host.classList.toggle("hidden", !open);
    host.hidden = !open;
    if (!open) {
      preview.replaceChildren();
      errorNode.hidden = true;
      errorNode.textContent = "";
      return;
    }
    state = ensureQueryKeys(table, state);
    const source = currentSource();
    if (state.mode === "scan" && source.kind === "local") state.source = "";
    paintSources();
    syncModeButtons();
    paintRows();
    preview.dataset.testid = "table-query-dql";
    refresh(true);
  }

  // The last answer for the current read, and the request that is still out, if any.
  let shown = { dql: "", error: "" };
  let previewSeq = 0;
  let previewTimer = null;
  let pending = null;
  let ranFollowsPreview = false;

  function showPreview(result) {
    shown = { dql: result.dql || "", error: result.error || "" };
    paintPreview(preview, shown.dql);
    errorNode.textContent = shown.error;
    errorNode.hidden = !shown.error;
    if (ranFollowsPreview) {
      ran = shown.dql;
      ranFollowsPreview = false;
    }
    runBtn.classList.toggle("is-pending", Boolean(shown.dql) && shown.dql !== ran);
    runBtn.disabled = Boolean(shown.error) || !shown.dql;
    host.dataset.preview = "ready";
    onChange({ table, state, read: readRequest(table, state, extra), dql: shown.dql, error: shown.error });
  }

  // Ask for the preview of the read as it is now. Only the newest answer is shown; an older one
  // that arrives late is dropped. Run waits for the answer that is out.
  function refresh(immediate) {
    if (!table || !table.name) return;
    if (!previewRead) {
      showPreview({ dql: statement(), error: error() });
      return;
    }
    const seq = ++previewSeq;
    runBtn.disabled = true;
    host.dataset.preview = "pending";
    clearTimeout(previewTimer);
    let settle;
    pending = new Promise((resolve) => {
      settle = resolve;
    });
    const ask = async () => {
      const read = readRequest(table, state, extra);
      let result;
      try {
        result = await previewRead(read);
      } catch (failure) {
        result = { dql: "", error: (failure && failure.message) || String(failure) };
      }
      if (seq !== previewSeq) {
        settle();
        return;
      }
      pending = null;
      showPreview(result);
      settle();
    };
    if (immediate) void ask();
    else previewTimer = setTimeout(ask, PREVIEW_DELAY_MS);
  }

  function rowList(kind) {
    return kind === "key" ? state.keys : state.filters;
  }

  // The hint under the Keys box says how many repeated keys the statement leaves out.
  function paintBatchHint() {
    const hint = batchKeyHint(table, state.source);
    let dropped = 0;
    if (state.mode === "keys") {
      const parsed = parseBatchKeys(state.batchKeys);
      if (!parsed.errors.length) {
        dropped = dedupeKeys(parsed.keys, table, extra, sourceOf(table, state.source, "query")).dropped;
      }
    }
    batchHint.textContent = dropped
      ? `${hint} · ${dropped} repeated ${dropped === 1 ? "key" : "keys"} left out`
      : hint;
  }

  function livePreview() {
    paintBatchHint();
    refresh(false);
  }

  function applyAttribute(kind, id, name, node) {
    const list = rowList(kind);
    const row = list.find((item) => item.id === id);
    if (!row || row.locked) return;
    row.attribute = name;
    if (kind === "key") {
      const key = sourceKeys(table, state.source, state.mode).find((item) => item.name === row.attribute);
      row.role = key ? key.role : "";
      const ops = opsFor("key", row.role, state.mode);
      if (!ops.some((op) => op.id === row.op)) row.op = ops[0].id;
      if (node) {
        const opSelect = node.querySelector("[data-field=op]");
        fillSelect(opSelect, ops.map((item) => ({ value: item.id, label: item.label })), row.op);
        opSelect.disabled = Boolean(row.locked && state.mode === "query" && row.role === "hash");
        syncValueFields(node, opById(row.op, ops));
      }
    }
    livePreview();
  }

  function updateRow(kind, id, patch) {
    const list = rowList(kind);
    const row = list.find((item) => item.id === id);
    if (!row) return;
    Object.assign(row, patch);
    if (patch.attribute && kind === "key") {
      const key = sourceKeys(table, state.source, state.mode).find((item) => item.name === row.attribute);
      row.role = key ? key.role : "";
      const ops = opsFor("key", row.role, state.mode);
      if (!ops.some((op) => op.id === row.op)) row.op = ops[0].id;
    }
    paint();
  }

  function bindRowEvents(kind, node) {
    node.addEventListener("change", (event) => {
      const field = event.target.dataset.field;
      const id = node.dataset.id;
      if (field === "attribute") applyAttribute(kind, id, event.target.value, node);
      else if (field === "op") updateRow(kind, id, { op: event.target.value });
      else if (field === "sizeOp") updateRow(kind, id, { sizeOp: event.target.value });
      else if (field === "type") updateRow(kind, id, { value: event.target.value });
    });
    node.addEventListener("input", (event) => {
      const field = event.target.dataset.field;
      const id = node.dataset.id;
      if (field === "attribute") applyAttribute(kind, id, event.target.value, node);
      else if (field === "value" || field === "value2") {
        const list = rowList(kind);
        const row = list.find((item) => item.id === id);
        if (!row) return;
        row[field] = event.target.value;
        livePreview();
      }
    });
    node.addEventListener("click", (event) => {
      const button = event.target.closest("[data-field=remove]");
      if (!button) return;
      const id = node.dataset.id;
      if (kind === "key") state.keys = state.keys.filter((row) => row.id !== id);
      else state.filters = state.filters.filter((row) => row.id !== id);
      paint();
    });
  }

  sourceSelect.addEventListener("change", () => {
    state.source = sourceSelect.value;
    if (state.mode !== "keys") state.keys = [];
    paint();
  });
  scanBtn.addEventListener("click", () => {
    state.mode = "scan";
    paint();
  });
  queryBtn.addEventListener("click", () => {
    state.mode = "query";
    paint();
  });
  keysModeBtn.addEventListener("click", () => {
    state.mode = "keys";
    paint();
    batchInput.focus();
  });
  batchInput.onInput(() => {
    state.batchKeys = batchInput.value;
    livePreview();
  });
  forwardBtn.addEventListener("click", () => {
    state.order = "forward";
    paint();
  });
  reverseBtn.addEventListener("click", () => {
    state.order = "reverse";
    paint();
  });
  addKey.addEventListener("click", () => {
    const next = unusedKeys()[0];
    const row = newRow(next ? next.name : "", "eq", {
      role: next ? next.role : "",
      locked: Boolean(next && state.mode === "query" && next.role === "hash"),
    });
    state.keys.push(row);
    focusAttr = row.id;
    paint();
  });
  addFilter.addEventListener("click", () => {
    const row = newRow("", "eq");
    state.filters.push(row);
    focusAttr = row.id;
    paint();
  });
  host.addEventListener("submit", async (event) => {
    event.preventDefault();
    // A keystroke may still be waiting for its preview. Run what the read is now.
    if (previewTimer || pending) {
      clearTimeout(previewTimer);
      previewTimer = null;
      refresh(true);
    }
    while (pending) await pending;
    if (shown.error) {
      errorNode.textContent = shown.error;
      errorNode.hidden = false;
      return;
    }
    if (!shown.dql) return;
    ran = shown.dql;
    runBtn.classList.remove("is-pending");
    onRun({ table, state, read: readRequest(table, state, extra), dql: ran });
  });

  paint();

  return {
    reset(nextTable) {
      table = nextTable || null;
      extra = new Map();
      state = defaultReadState();
      ran = "";
      ranFollowsPreview = Boolean(table && table.name);
      paint();
      runBtn.classList.remove("is-pending");
    },
    updateTable(nextTable) {
      if (!nextTable) return;
      table = nextTable;
      paint();
    },
    addAttributes(names) {
      let added = false;
      for (const name of names || []) {
        if (!name) continue;
        if (!extra.has(name)) added = true;
        extra.set(name, extra.get(name) || "");
      }
      if (added) refreshAttributeLists();
    },
    markRan(dql) {
      ran = dql || shown.dql;
      runBtn.classList.toggle("is-pending", shown.dql !== ran);
    },
    statement: () => shown.dql,
    error: () => shown.error,
    read: () => readRequest(table, state, extra),
    state() {
      return {
        ...state,
        keys: state.keys.map((row) => ({ ...row })),
        filters: state.filters.map((row) => ({ ...row })),
      };
    },
  };
}
