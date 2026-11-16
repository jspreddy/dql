// Table browser read picker: SCAN/SELECT from source, keys, filters, and order.

import { highlightParts, knownWords } from "./dql-tokens.js";

const RESERVED = new Set(knownWords());

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
];

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

export function quoteIdent(name) {
  const text = String(name || "");
  if (IDENT.test(text) && !RESERVED.has(text.toUpperCase())) return text;
  return "'" + text.replaceAll("'", "''") + "'";
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

export function opById(id, ops) {
  return (ops || FILTER_OPS).find((op) => op.id === id) || COMPARE_OPS[0];
}

export function opsFor(kind, role, mode) {
  if (kind === "filter") return FILTER_OPS;
  if (mode === "query" && role === "hash") return KEY_HASH_OPS;
  if (mode === "query") return KEY_RANGE_OPS;
  return KEY_SCAN_OPS;
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
  const sources = [tableSource(table)];
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

export function filterAttributes(table, extraNames) {
  const names = [];
  const seen = new Set();
  const add = (name, type, role) => {
    if (!name || seen.has(name)) return;
    seen.add(name);
    names.push({ name, type: type || "", role: role || "" });
  };
  for (const key of tableSource(table).hashKey ? sourceKeys(table, "", "query") : []) {
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

export function formatCondition(row, table, extra) {
  if (!row || !row.attribute) return "";
  const ops = opsFor(row.kind || "filter", row.role, row.mode);
  const op = opById(row.op, ops);
  const field = quoteIdent(row.attribute);
  const type = attributeType(table, extra, row.attribute);
  const values = op.values === 0 ? 0 : op.values === 2 ? 2 : 1;
  if (values === 1 && !String(row.value ?? "").length && !op.type) return "";
  if (values === 2 && (!String(row.value ?? "").length || !String(row.value2 ?? "").length)) return "";
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
  if (!hash) return next;
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

export function buildReadStatement(tableName, table, state, extra) {
  const name = tableName || (table && table.name) || "";
  if (!name) return "";
  const current = ensureQueryKeys(table, state || defaultReadState());
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
  if (source.name) sql += " USING " + quoteIdent(source.name);
  if (current.order === "reverse") {
    if (current.mode === "query") sql += " DESC";
    else {
      const field = source.rangeKey || source.hashKey;
      if (field) sql += " ORDER BY " + quoteIdent(field) + " DESC";
    }
  }
  return sql;
}

export function readError(table, state) {
  const current = ensureQueryKeys(table, state || defaultReadState());
  if (current.mode !== "query") return "";
  const hash = sourceKeys(table, current.source, current.mode).find((key) => key.role === "hash");
  if (!hash) return "Query needs a hash key in the table definition.";
  const row = (current.keys || []).find((item) => item.attribute === hash.name);
  if (!row || !String(row.value ?? "").length) return "Query needs a hash key value.";
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

export function attachTableQuery(host, handlers = {}) {
  const onRun = handlers.onRun || (() => {});
  const onChange = handlers.onChange || (() => {});
  let table = null;
  let extra = new Map();
  let state = defaultReadState();
  let ran = "";

  const sourceSelect = host.querySelector("#table-query-source");
  const scanBtn = host.querySelector("#table-query-scan");
  const queryBtn = host.querySelector("#table-query-query");
  const forwardBtn = host.querySelector("#table-query-forward");
  const reverseBtn = host.querySelector("#table-query-reverse");
  const runBtn = host.querySelector("#table-query-run");
  const addKey = host.querySelector("#table-query-add-key");
  const addFilter = host.querySelector("#table-query-add-filter");
  const keyHost = host.querySelector("#table-query-keys");
  const filterHost = host.querySelector("#table-query-filters");
  const preview = host.querySelector("#table-query-dql");
  const errorNode = host.querySelector("#table-query-error");

  function currentSource() {
    return sourceOf(table, state.source, state.mode);
  }

  function statement() {
    return buildReadStatement(table && table.name, table, state, extra);
  }

  function error() {
    return readError(table, state);
  }

  function unusedKeys() {
    const used = new Set((state.keys || []).map((row) => row.attribute));
    return sourceKeys(table, state.source, state.mode).filter((key) => !used.has(key.name));
  }

  function syncModeButtons() {
    scanBtn.setAttribute("aria-pressed", state.mode === "scan" ? "true" : "false");
    queryBtn.setAttribute("aria-pressed", state.mode === "query" ? "true" : "false");
    forwardBtn.setAttribute("aria-pressed", state.order === "forward" ? "true" : "false");
    reverseBtn.setAttribute("aria-pressed", state.order === "reverse" ? "true" : "false");
  }

  function paintSources() {
    const selected = state.source || "";
    const items = availableSources(table, state.mode).map((source) => {
      if (!source.name) return { value: "", label: "Table" };
      const keys = [source.hashKey, source.rangeKey].filter(Boolean).join(", ");
      const kind = source.kind === "local" ? "local" : "global";
      return { value: source.name, label: keys ? `${source.name} (${kind}: ${keys})` : `${source.name} (${kind})` };
    });
    fillSelect(sourceSelect, items, selected);
    if (![...sourceSelect.options].some((node) => node.value === selected)) {
      state.source = "";
      sourceSelect.value = "";
    }
  }

  function paintConditionRow(kind, row) {
    const wrap = document.createElement("div");
    wrap.className = "table-query-cond";
    wrap.dataset.kind = kind;
    wrap.dataset.id = row.id;
    wrap.dataset.testid = kind === "key" ? "table-query-key-row" : "table-query-filter-row";

    const keys = kind === "key"
      ? sourceKeys(table, state.source, state.mode)
      : filterAttributes(table, extra);
    const role = row.role || (keys.find((key) => key.name === row.attribute) || {}).role || "";
    const ops = opsFor(kind, role, state.mode);
    const attr = document.createElement("select");
    attr.dataset.field = "attribute";
    attr.setAttribute("aria-label", kind === "key" ? "Key attribute" : "Filter attribute");
    attr.dataset.testid = "table-query-attr";
    const attrItems = (keys.length ? keys : [{ name: row.attribute || "" }]).map((item) => ({
      value: item.name,
      label: item.role === "hash" ? `${item.name} (hash)` : item.role === "range" ? `${item.name} (range)` : item.name,
    }));
    fillSelect(attr, attrItems.filter((item) => item.value), row.attribute);
    if (row.locked) attr.disabled = true;

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

    wrap.append(attr, op, value, value2, type, remove);
    syncValueFields(wrap, opById(row.op, ops));
    return wrap;
  }

  function syncValueFields(wrap, op) {
    const values = op.values === 0 ? 0 : op.values === 2 ? 2 : 1;
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
    addKey.disabled = unusedKeys().length === 0;
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
    const sql = statement();
    paintPreview(preview, sql);
    preview.dataset.testid = "table-query-dql";
    const message = error();
    errorNode.textContent = message;
    errorNode.hidden = !message;
    runBtn.classList.toggle("is-pending", Boolean(sql) && sql !== ran);
    runBtn.disabled = Boolean(message);
    onChange({ table, state, dql: sql, error: message });
  }

  function rowList(kind) {
    return kind === "key" ? state.keys : state.filters;
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
      if (field === "attribute") updateRow(kind, id, { attribute: event.target.value });
      else if (field === "op") updateRow(kind, id, { op: event.target.value });
      else if (field === "type") updateRow(kind, id, { value: event.target.value });
    });
    node.addEventListener("input", (event) => {
      const field = event.target.dataset.field;
      const id = node.dataset.id;
      if (field === "value" || field === "value2") {
        const list = rowList(kind);
        const row = list.find((item) => item.id === id);
        if (!row) return;
        row[field] = event.target.value;
        const sql = statement();
        paintPreview(preview, sql);
        const message = error();
        errorNode.textContent = message;
        errorNode.hidden = !message;
        runBtn.classList.toggle("is-pending", Boolean(sql) && sql !== ran);
        runBtn.disabled = Boolean(message);
        onChange({ table, state, dql: sql, error: message });
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
    state.keys = [];
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
    if (!next) return;
    state.keys.push(newRow(next.name, state.mode === "query" && next.role === "hash" ? "eq" : "eq", {
      role: next.role,
      locked: state.mode === "query" && next.role === "hash",
    }));
    paint();
  });
  addFilter.addEventListener("click", () => {
    const attrs = filterAttributes(table, extra);
    state.filters.push(newRow(attrs[0] ? attrs[0].name : "", "eq"));
    paint();
  });
  host.addEventListener("submit", (event) => {
    event.preventDefault();
    const message = error();
    if (message) {
      errorNode.textContent = message;
      errorNode.hidden = false;
      return;
    }
    ran = statement();
    runBtn.classList.remove("is-pending");
    onRun({ table, state, dql: ran });
  });

  paint();

  return {
    reset(nextTable) {
      table = nextTable || null;
      extra = new Map();
      state = defaultReadState();
      paint();
      ran = table && table.name ? statement() : "";
      runBtn.classList.remove("is-pending");
    },
    updateTable(nextTable) {
      if (!nextTable) return;
      table = nextTable;
      paint();
    },
    addAttributes(names) {
      for (const name of names || []) {
        if (name) extra.set(name, extra.get(name) || "");
      }
      paint();
    },
    markRan(dql) {
      ran = dql || statement();
      runBtn.classList.toggle("is-pending", statement() !== ran);
    },
    statement,
    error,
    state() {
      return { ...state, keys: state.keys.map((row) => ({ ...row })), filters: state.filters.map((row) => ({ ...row })) };
    },
  };
}
