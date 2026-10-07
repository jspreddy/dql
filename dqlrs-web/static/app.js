import { Compartment, EditorState } from "@codemirror/state";
import {
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
  placeholder,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";
import { dqlLanguage, queryHighlight } from "./dql-mode.js";

const dqlHighlight = HighlightStyle.define([
  { tag: tags.keyword, color: "#0f766e", fontWeight: "650" },
  { tag: tags.deleted, color: "#dc2626", fontWeight: "700" },
  { tag: tags.typeName, color: "#1d4ed8" },
  { tag: tags.function(tags.variableName), color: "#6d28d9" },
  { tag: tags.bool, color: "#0369a1", fontWeight: "650" },
  { tag: tags.null, color: "#0369a1", fontWeight: "650" },
  { tag: tags.string, color: "#9f1239" },
  { tag: tags.number, color: "#b45309" },
  { tag: tags.comment, color: "#6b7280", fontStyle: "italic" },
  { tag: tags.operator, color: "#334155" },
]);

const state = {
  mode: "query",
  path: "",
  expanded: new Set(["."]),
  expandedOnce: false,
  saveTimer: 0,
  searchTimer: 0,
  table: "",
  page: 0,
  hasMore: false,
  suppressSave: false,
};

const HIGHLIGHT_KEY = "dqlrs-web.highlight";

function loadHighlightOptions() {
  try {
    const parsed = JSON.parse(localStorage.getItem(HIGHLIGHT_KEY) || "{}");
    return {
      minimalWrite: Boolean(parsed.minimalWrite),
      evenOdd: parsed.evenOdd !== false,
    };
  } catch {
    return { minimalWrite: false, evenOdd: true };
  }
}

function saveHighlightOptions(options) {
  localStorage.setItem(HIGHLIGHT_KEY, JSON.stringify(options));
}

const highlightOptions = loadHighlightOptions();
const highlightCompartment = new Compartment();
const treeEl = document.querySelector("#tree");
const editor = new EditorView({
  parent: document.querySelector("#editor"),
  state: EditorState.create({
    doc: "",
    extensions: [
      lineNumbers(),
      highlightActiveLine(),
      highlightActiveLineGutter(),
      history(),
      keymap.of([indentWithTab, ...defaultKeymap, ...historyKeymap]),
      dqlLanguage,
      highlightCompartment.of(queryHighlight(highlightOptions)),
      syntaxHighlighting(dqlHighlight),
      placeholder("Open a .dql file, or create one."),
      EditorView.lineWrapping,
      EditorView.contentAttributes.of({ spellcheck: "false" }),
      EditorView.theme({
        "&": { height: "100%", fontSize: "12.5px" },
        "&.cm-focused": { outline: "none" },
        ".cm-scroller": {
          fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
          lineHeight: "1.4",
        },
        ".cm-gutters": { background: "#fafbfc", color: "#8b97a3", border: "none" },
        ".cm-line.cm-dql-alt": { backgroundColor: "#f4f7f8" },
        ".cm-line.cm-dql-write": { backgroundColor: "#fdecec" },
        ".cm-line.cm-dql-write-even": { backgroundColor: "#f3c4c4" },
        ".cm-dql-gutter": { width: "8px", background: "#fafbfc" },
        ".cm-dql-gutter .cm-gutterElement": { padding: "0 0 0 3px" },
        ".cm-write-bar": {
          display: "block",
          width: "3px",
          height: "100%",
          borderRadius: "1px",
          backgroundColor: "#fb7185",
        },
        ".cm-write-bar-even": { backgroundColor: "#be123c" },
        ".cm-activeLine": { backgroundColor: "transparent" },
        ".cm-activeLineGutter": { background: "#f3f5f7" },
        ".cm-content": { padding: "4px 0" },
        ".cm-line": { padding: "0 8px" },
        ".cm-placeholder": { color: "#8b97a3", fontStyle: "italic" },
      }),
      EditorView.updateListener.of((update) => {
        if (update.docChanged && !state.suppressSave) scheduleSave();
      }),
    ],
  }),
});
const resultBody = document.querySelector("#result-body");
const resultMeta = document.querySelector("#result-meta");

document.querySelector("#mode-query").addEventListener("click", () => setMode("query"));
document.querySelector("#mode-tables").addEventListener("click", () => setMode("tables"));
document.querySelector("#new-file").addEventListener("click", createFile);
document.querySelector("#run").addEventListener("click", runSelection);
const minimalWriteInput = document.querySelector("#opt-minimal-write");
const evenOddInput = document.querySelector("#opt-even-odd");
minimalWriteInput.checked = highlightOptions.minimalWrite;
evenOddInput.checked = highlightOptions.evenOdd;
minimalWriteInput.addEventListener("change", applyHighlightToggles);
evenOddInput.addEventListener("change", applyHighlightToggles);

function applyHighlightToggles() {
  const options = {
    minimalWrite: minimalWriteInput.checked,
    evenOdd: evenOddInput.checked,
  };
  saveHighlightOptions(options);
  editor.dispatch({
    effects: highlightCompartment.reconfigure(queryHighlight(options)),
  });
}
document.querySelector("#table-search").addEventListener("input", () => {
  clearTimeout(state.searchTimer);
  state.searchTimer = setTimeout(loadTables, 250);
});
document.querySelector("#page-prev").addEventListener("click", () => changePage(-1));
document.querySelector("#page-next").addEventListener("click", () => changePage(1));

boot();

async function boot() {
  const config = await api("/api/config");
  document.querySelector("#endpoint").textContent = config.endpoint;
  await refreshTree();
  if (location.hash === "#tables") {
    setMode("tables");
    return;
  }
  const file = treeEl.querySelector('.node[data-kind="file"]');
  if (file) file.click();
}

function setMode(mode) {
  state.mode = mode;
  document.querySelector("#query-view").classList.toggle("hidden", mode !== "query");
  document.querySelector("#tables-view").classList.toggle("hidden", mode !== "tables");
  document.querySelector("#mode-query").classList.toggle("active", mode === "query");
  document.querySelector("#mode-tables").classList.toggle("active", mode === "tables");
  document.querySelector("#mode-query").setAttribute("aria-selected", mode === "query");
  document.querySelector("#mode-tables").setAttribute("aria-selected", mode === "tables");
  if (mode === "tables") loadTables();
}

async function refreshTree() {
  const root = await api("/api/tree");
  if (!state.expandedOnce) {
    expandAll(root);
    state.expandedOnce = true;
  }
  treeEl.replaceChildren(renderDir(root, true));
}

function expandAll(node) {
  if (node.type === "dir") state.expanded.add(node.path);
  for (const child of node.children || []) expandAll(child);
}

function renderDir(node, isRoot) {
  const list = document.createElement("ul");
  for (const child of node.children || []) {
    const item = document.createElement("li");
    if (child.type === "dir") {
      const open = state.expanded.has(child.path);
      const button = nodeButton(open ? "▾" : "▸", child.name, false);
      button.dataset.kind = "dir";
      button.addEventListener("click", () => {
        if (state.expanded.has(child.path)) state.expanded.delete(child.path);
        else state.expanded.add(child.path);
        refreshTree();
      });
      item.append(button);
      if (open) item.append(renderDir(child, false));
    } else {
      const button = nodeButton("", child.name, child.path === state.path);
      button.dataset.kind = "file";
      button.dataset.path = child.path;
      button.addEventListener("click", () => openFile(child.path));
      item.append(button);
    }
    list.append(item);
  }
  if (isRoot && !(node.children || []).length) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = "No .dql files yet";
    list.append(empty);
  }
  return list;
}

function nodeButton(twist, label, selected) {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "node" + (selected ? " selected" : "");
  const marker = document.createElement("span");
  marker.className = "twist";
  marker.textContent = twist || " ";
  button.append(marker, document.createTextNode(label));
  return button;
}

async function openFile(path) {
  await flushSave();
  const file = await api("/api/file?path=" + encodeURIComponent(path));
  state.path = file.path;
  setEditorText(file.text);
  document.querySelector("#file-name").textContent = file.path.split("/").pop();
  setMode("query");
  await refreshTree();
}

async function createFile() {
  const entered = window.prompt("New file path", "queries/notes.dql");
  if (!entered) return;
  const created = await api("/api/file", { method: "POST", body: { path: entered } });
  const parent = created.path.includes("/") ? created.path.slice(0, created.path.lastIndexOf("/")) : ".";
  state.expanded.add(parent);
  await openFile(created.path);
}

function scheduleSave() {
  clearTimeout(state.saveTimer);
  state.saveTimer = setTimeout(flushSave, 400);
}

async function flushSave() {
  clearTimeout(state.saveTimer);
  if (!state.path) return;
  await api("/api/file", { method: "PUT", body: { path: state.path, text: editorText() } });
}

function editorText() {
  return editor.state.doc.toString();
}

function setEditorText(text) {
  state.suppressSave = true;
  editor.dispatch({
    changes: { from: 0, to: editor.state.doc.length, insert: text },
    selection: { anchor: 0 },
  });
  state.suppressSave = false;
}

async function runSelection() {
  await flushSave();
  const selection = editor.state.selection.main;
  const selected = selection.empty ? "" : editor.state.sliceDoc(selection.from, selection.to);
  const dql = selected.trim() ? selected : editorText();
  const run = document.querySelector("#run");
  run.disabled = true;
  resultMeta.textContent = "Running…";
  resultBody.replaceChildren();
  try {
    const payload = await api("/api/run", { method: "POST", body: { dql } });
    renderResults(payload.results || []);
  } catch (error) {
    resultMeta.textContent = "";
    resultBody.append(note(error.message, true));
  } finally {
    run.disabled = false;
  }
}

function renderResults(results) {
  resultBody.replaceChildren();
  const lastItems = [...results].reverse().find((item) => item.kind === "items" && item.ok);
  const notes = results.filter((item) => item !== lastItems);
  for (const item of notes) {
    resultBody.append(note(resultLabel(item), !item.ok));
  }
  if (lastItems) {
    const rows = lastItems.items || [];
    resultMeta.textContent = rows.length + (rows.length === 1 ? " row" : " rows");
    resultBody.append(dataTable(rows));
  } else {
    resultMeta.textContent = results.length ? "" : "Nothing to run";
  }
}

function resultLabel(item) {
  if (!item.ok) return item.error || item.message || "failed";
  if (item.kind === "affected") return (item.affected ?? 0) + " affected";
  if (item.kind === "status" || item.kind === "schema" || item.kind === "text") return item.message || item.kind;
  if (item.kind === "items") return ((item.items || []).length) + " rows";
  return item.message || "ok";
}

function note(text, isError) {
  const div = document.createElement("div");
  div.className = "note" + (isError ? " error" : "");
  div.textContent = text;
  return div;
}

function dataTable(rows) {
  if (!rows.length) return note("No rows", false);
  const keys = [];
  for (const row of rows) {
    for (const key of Object.keys(row)) {
      if (!keys.includes(key)) keys.push(key);
    }
  }
  const table = document.createElement("table");
  const head = document.createElement("tr");
  for (const key of keys) {
    const cell = document.createElement("th");
    cell.textContent = key;
    head.append(cell);
  }
  const thead = document.createElement("thead");
  thead.append(head);
  const body = document.createElement("tbody");
  for (const row of rows) {
    const tr = document.createElement("tr");
    for (const key of keys) {
      const cell = document.createElement("td");
      const value = row[key];
      cell.textContent = value === null || value === undefined ? "" : typeof value === "object" ? JSON.stringify(value) : String(value);
      tr.append(cell);
    }
    body.append(tr);
  }
  table.append(thead, body);
  return table;
}

async function loadTables() {
  const pattern = document.querySelector("#table-search").value.trim();
  const list = document.querySelector("#table-list");
  list.replaceChildren(note("Loading…", false));
  try {
    const payload = await api("/api/tables?pattern=" + encodeURIComponent(pattern));
    list.replaceChildren();
    if (!payload.tables.length) {
      list.append(note(pattern ? "No tables match " + pattern : "No tables", false));
      return;
    }
    if (!payload.tables.some((table) => table.name === state.table)) {
      state.table = payload.tables[0].name;
      state.page = 0;
    }
    for (const table of payload.tables) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "table-row" + (table.name === state.table ? " selected" : "");
      const name = document.createElement("strong");
      name.textContent = table.name;
      const keys = document.createElement("span");
      keys.className = "keys";
      keys.textContent = table.keys || "";
      button.append(name, keys);
      button.addEventListener("click", () => {
        state.table = table.name;
        state.page = 0;
        loadTables();
      });
      list.append(button);
    }
    await loadRows();
  } catch (error) {
    list.replaceChildren(note(error.message, true));
  }
}

async function loadRows() {
  if (!state.table) return;
  const body = document.querySelector("#rows-body");
  document.querySelector("#rows-title").textContent = state.table;
  document.querySelector("#rows-meta").textContent = "50 rows at a time";
  body.replaceChildren(note("Loading…", false));
  try {
    const payload = await api(
      "/api/rows?table=" + encodeURIComponent(state.table) + "&page=" + state.page
    );
    state.hasMore = Boolean(payload.has_more);
    const start = payload.page * payload.page_size + (payload.items.length ? 1 : 0);
    const end = payload.page * payload.page_size + payload.items.length;
    document.querySelector("#page-label").textContent = payload.items.length
      ? "Showing " + start + "–" + end
      : "No rows";
    document.querySelector("#page-prev").disabled = payload.page <= 0;
    document.querySelector("#page-next").disabled = !state.hasMore;
    body.replaceChildren(dataTable(payload.items || []));
    body.scrollTop = 0;
  } catch (error) {
    body.replaceChildren(note(error.message, true));
  }
}

function changePage(delta) {
  const next = state.page + delta;
  if (next < 0 || (delta > 0 && !state.hasMore)) return;
  state.page = next;
  loadRows();
}

async function api(url, options = {}) {
  const response = await fetch(url, {
    method: options.method || "GET",
    headers: options.body ? { "Content-Type": "application/json" } : undefined,
    body: options.body ? JSON.stringify(options.body) : undefined,
  });
  const payload = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(payload.error || response.statusText);
  return payload;
}
