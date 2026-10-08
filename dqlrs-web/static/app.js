import { Compartment, EditorState, Prec } from "@codemirror/state";
import {
  drawSelection,
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
import { bandLayer, dqlLanguage, markersForRunStatus, queryHighlight, runFrame, runStatusField, runStatusGutter, setRunStatuses } from "./dql-mode.js";
import { runTarget, statementSpans } from "./dql-tokens.js";

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

let runMarks = [];

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
  dragPath: "",
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
      runStatusField,
      runStatusGutter,
      highlightActiveLine(),
      highlightActiveLineGutter(),
      history(),
      keymap.of([indentWithTab, ...defaultKeymap, ...historyKeymap]),
      Prec.high(keymap.of([{ key: "Mod-Enter", run: runFromEditor }])),
      dqlLanguage,
      highlightCompartment.of(queryHighlight(highlightOptions)),
      runFrame,
      bandLayer,
      syntaxHighlighting(dqlHighlight),
      placeholder("Open a .dql file, or create one."),
      EditorView.lineWrapping,
      EditorView.contentAttributes.of({ spellcheck: "false" }),
      drawSelection(),
      Prec.highest(EditorView.theme({
        ".cm-selectionBackground, &.cm-focused .cm-selectionLayer .cm-selectionBackground": {
          backgroundColor: "transparent !important",
        },
        ".cm-run-fill": {
          backgroundColor: "rgba(16, 42, 96, 0.05)",
          border: "1px solid #2563eb",
          boxSizing: "content-box",
        },
        "&.cm-focused .cm-content ::selection, &.cm-focused .cm-content::selection, .cm-line ::selection, .cm-line::selection": {
          backgroundColor: "transparent !important",
          color: "inherit !important",
        },
      })),
      EditorView.theme({
        "&": { height: "100%", fontSize: "12.5px" },
        "&.cm-focused": { outline: "none" },
        ".cm-scroller": {
          fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
          lineHeight: "1.4",
        },
        ".cm-gutters": { background: "#fafbfc", color: "#8b97a3", border: "none" },
        ".cm-run-status-gutter": { width: "16px" },
        ".cm-run-status-gutter .cm-gutterElement": { padding: "0 1px" },
        ".cm-band-alt": { backgroundColor: "#f4f7f8" },
        ".cm-band-write": { backgroundColor: "#fdecec" },
        ".cm-band-write-even": { backgroundColor: "#f3c4c4" },
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
        ".cm-line": { padding: "0 8px", backgroundColor: "transparent" },
        ".cm-placeholder": { color: "#8b97a3", fontStyle: "italic" },
      }),
      EditorView.updateListener.of((update) => {
        if (update.docChanged && !state.suppressSave) scheduleSave();
        if (update.docChanged && runMarks.length) {
          runMarks = runMarks.map((mark) => ({ ...mark, from: update.changes.mapPos(mark.from, 1) }));
        }
      }),
    ],
  }),
});
const resultBody = document.querySelector("#result-body");
const runProgress = document.querySelector("#run-progress");
const resultMeta = document.querySelector("#result-meta");

document.querySelector("#mode-query").addEventListener("click", () => setMode("query"));
document.querySelector("#mode-tables").addEventListener("click", () => setMode("tables"));
document.querySelector("#new-file").addEventListener("click", createFile);
const runButton = document.querySelector("#run");
const runShortcut = document.querySelector("#run-shortcut");
const runShortcutLabel = /Mac|iPhone|iPad|iPod/.test(navigator.platform || navigator.userAgent) ? "⌘Enter" : "Ctrl+Enter";
runShortcut.textContent = runShortcutLabel;
runButton.title = "Run selection (" + runShortcutLabel + ")";
runButton.setAttribute("aria-keyshortcuts", "Control+Enter");
runButton.addEventListener("click", runSelection);

function runFromEditor() {
  if (runButton.disabled) return true;
  runSelection();
  return true;
}
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
    const open = child.type === "dir" && state.expanded.has(child.path);
    const button = nodeButton(child.type === "dir" ? (open ? "▾" : "▸") : "", child.name, child.path === state.path);
    button.dataset.kind = child.type;
    button.dataset.path = child.path;
    bindNode(button, child);
    item.append(button);
    if (open) item.append(renderDir(child, false));
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

function bindNode(button, child) {
  let clickTimer = 0;
  button.addEventListener("click", () => {
    if (button.querySelector("input")) return;
    clearTimeout(clickTimer);
    clickTimer = window.setTimeout(() => {
      if (child.type === "dir") toggleDir(child.path);
      else openFile(child.path);
    }, 220);
  });
  button.addEventListener("dblclick", (event) => {
    event.preventDefault();
    clearTimeout(clickTimer);
    beginRename(child.path);
  });
  button.addEventListener("contextmenu", (event) => {
    event.preventDefault();
    event.stopPropagation();
    showMenu(event.clientX, event.clientY, menuItems(child));
  });
  button.draggable = true;
  button.addEventListener("dragstart", (event) => {
    if (button.querySelector("input")) {
      event.preventDefault();
      return;
    }
    state.dragPath = child.path;
    event.dataTransfer.setData("text/plain", child.path);
    event.dataTransfer.effectAllowed = "move";
  });
  button.addEventListener("dragend", () => {
    state.dragPath = "";
    clearDropTargets();
  });
  button.addEventListener("dragover", (event) => {
    if (!state.dragPath) return;
    event.stopPropagation();
    if (!canDropOn(child)) return;
    event.preventDefault();
    button.classList.add("drop-target");
    event.dataTransfer.dropEffect = "move";
  });
  button.addEventListener("dragleave", () => button.classList.remove("drop-target"));
  button.addEventListener("drop", (event) => {
    if (!state.dragPath) return;
    event.preventDefault();
    event.stopPropagation();
    button.classList.remove("drop-target");
    if (!canDropOn(child)) return;
    const destDir = child.type === "dir" ? child.path : parentOf(child.path);
    dropOnto(state.dragPath, destDir);
  });
}

function toggleDir(path) {
  if (state.expanded.has(path)) state.expanded.delete(path);
  else state.expanded.add(path);
  refreshTree();
}

function parentOf(path) {
  const index = path.lastIndexOf("/");
  return index < 0 ? "." : path.slice(0, index);
}

function joinPath(dir, name) {
  const clean = name.replace(/^[/\\]+/, "");
  return !dir || dir === "." ? clean : dir + "/" + clean;
}

function canDropOn(child) {
  const from = state.dragPath;
  if (!from || from === child.path) return false;
  const destDir = child.type === "dir" ? child.path : parentOf(child.path);
  if (parentOf(from) === destDir) return false;
  if (destDir === from || destDir.startsWith(from + "/")) return false;
  return true;
}

function clearDropTargets() {
  treeEl.classList.remove("drop-target");
  for (const node of treeEl.querySelectorAll(".drop-target")) node.classList.remove("drop-target");
}

async function dropOnto(from, destDir) {
  if (!from) return;
  const name = from.split("/").pop();
  const to = destDir === "." ? name : destDir + "/" + name;
  if (to === from) return;
  try {
    await relocate(from, to);
  } catch (error) {
    window.alert(error.message);
  }
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
  await createNamedFile(".");
}

async function createNamedFile(dir) {
  const entered = window.prompt(dir === "." ? "New file path" : "New file name", dir === "." ? "queries/notes.dql" : "notes.dql");
  if (!entered || !entered.trim()) return;
  const path = dir === "." ? entered.trim() : joinPath(dir, entered.trim());
  try {
    const created = await api("/api/file", { method: "POST", body: { path } });
    expandParents(parentOf(created.path));
    await openFile(created.path);
  } catch (error) {
    window.alert(error.message);
  }
}

async function createNamedFolder(dir) {
  const entered = window.prompt("New folder name", "folder");
  if (!entered || !entered.trim()) return;
  const path = dir === "." ? entered.trim() : joinPath(dir, entered.trim());
  try {
    const created = await api("/api/mkdir", { method: "POST", body: { path } });
    expandParents(created.path);
    state.expanded.add(created.path);
    await refreshTree();
  } catch (error) {
    window.alert(error.message);
  }
}

async function duplicateEntry(path) {
  try {
    await flushSave();
    const created = await api("/api/duplicate", { method: "POST", body: { path } });
    expandParents(parentOf(created.path));
    if (created.kind === "file") await openFile(created.path);
    else {
      state.expanded.add(created.path);
      await refreshTree();
    }
  } catch (error) {
    window.alert(error.message);
  }
}

async function moveEntry(path) {
  const entered = window.prompt("Move to", path);
  if (!entered || !entered.trim() || entered.trim() === path) return;
  try {
    await relocate(path, entered.trim());
  } catch (error) {
    window.alert(error.message);
  }
}

async function deleteEntry(path, kind) {
  const label = path.split("/").pop();
  const message = kind === "dir" ? "Delete folder " + label + " and the .dql files inside it?" : "Delete " + label + "?";
  if (!window.confirm(message)) return;
  clearTimeout(state.saveTimer);
  const removingOpen = state.path === path || (kind === "dir" && state.path.startsWith(path + "/"));
  if (removingOpen) {
    state.path = "";
    setEditorText("");
    document.querySelector("#file-name").textContent = "No file open";
  }
  try {
    await api("/api/delete", { method: "POST", body: { path } });
    await refreshTree();
  } catch (error) {
    window.alert(error.message);
    await refreshTree();
  }
}

async function relocate(from, to) {
  await flushSave();
  const moved = await api("/api/move", { method: "POST", body: { from, to } });
  if (state.path === from || state.path.startsWith(from + "/")) {
    state.path = moved.path + state.path.slice(from.length);
    document.querySelector("#file-name").textContent = state.path.split("/").pop();
  }
  remapExpanded(from, moved.path);
  expandParents(parentOf(moved.path));
  if (moved.kind === "dir") state.expanded.add(moved.path);
  await refreshTree();
}

function remapExpanded(from, to) {
  const next = new Set();
  for (const path of state.expanded) {
    if (path === from) next.add(to);
    else if (path.startsWith(from + "/")) next.add(to + path.slice(from.length));
    else next.add(path);
  }
  state.expanded = next;
}

function expandParents(path) {
  if (!path || path === ".") {
    state.expanded.add(".");
    return;
  }
  const parts = path.split("/");
  for (let i = 0; i < parts.length; i += 1) state.expanded.add(parts.slice(0, i + 1).join("/"));
}

function beginRename(path) {
  const button = [...treeEl.querySelectorAll(".node")].find((node) => node.dataset.path === path);
  if (!button || button.querySelector("input")) return;
  const name = path.split("/").pop();
  const input = document.createElement("input");
  input.className = "rename";
  input.value = name;
  input.spellcheck = false;
  input.setAttribute("aria-label", "Rename");
  const twist = button.querySelector(".twist");
  button.draggable = false;
  button.replaceChildren(twist, input);
  input.focus();
  const dot = name.lastIndexOf(".");
  if (button.dataset.kind === "file" && dot > 0) input.setSelectionRange(0, dot);
  else input.select();
  let settled = false;
  const finish = async (commit) => {
    if (settled) return;
    settled = true;
    const nextName = input.value.trim();
    if (!commit || !nextName || nextName === name || /[\\/]/.test(nextName)) {
      await refreshTree();
      return;
    }
    try {
      await relocate(path, joinPath(parentOf(path), nextName));
    } catch (error) {
      window.alert(error.message);
      await refreshTree();
    }
  };
  input.addEventListener("keydown", (event) => {
    event.stopPropagation();
    if (event.key === "Enter") {
      event.preventDefault();
      finish(true);
    } else if (event.key === "Escape") {
      event.preventDefault();
      finish(false);
    }
  });
  input.addEventListener("blur", () => finish(true));
  input.addEventListener("click", (event) => event.stopPropagation());
  input.addEventListener("dblclick", (event) => event.stopPropagation());
  input.addEventListener("mousedown", (event) => event.stopPropagation());
}

function menuItems(target) {
  const dir = target.type === "dir" ? target.path : parentOf(target.path || ".");
  const items = [
    { label: "New file", run: () => createNamedFile(dir) },
    { label: "New folder", run: () => createNamedFolder(dir) },
  ];
  if (!target.path || target.type === "root") return items;
  items.push(
    "-",
    { label: "Duplicate", run: () => duplicateEntry(target.path) },
    { label: "Rename", run: () => beginRename(target.path) },
    { label: "Move to…", run: () => moveEntry(target.path) },
    "-",
    { label: "Delete", danger: true, run: () => deleteEntry(target.path, target.type) },
  );
  return items;
}

const menuEl = document.querySelector("#file-menu");

function showMenu(x, y, items) {
  menuEl.replaceChildren();
  for (const item of items) {
    if (item === "-") {
      menuEl.append(document.createElement("hr"));
      continue;
    }
    const button = document.createElement("button");
    button.type = "button";
    button.role = "menuitem";
    button.textContent = item.label;
    if (item.danger) button.className = "danger";
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      hideMenu();
      item.run();
    });
    menuEl.append(button);
  }
  menuEl.hidden = false;
  menuEl.style.left = "0px";
  menuEl.style.top = "0px";
  const rect = menuEl.getBoundingClientRect();
  menuEl.style.left = Math.max(4, Math.min(x, window.innerWidth - rect.width - 8)) + "px";
  menuEl.style.top = Math.max(4, Math.min(y, window.innerHeight - rect.height - 8)) + "px";
}

function hideMenu() {
  menuEl.hidden = true;
}

document.addEventListener("click", (event) => {
  if (!menuEl.contains(event.target)) hideMenu();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") hideMenu();
});
document.querySelector(".side").addEventListener("contextmenu", (event) => {
  if (event.target.closest(".node")) return;
  event.preventDefault();
  showMenu(event.clientX, event.clientY, menuItems({ type: "root", path: "." }));
});
treeEl.addEventListener("scroll", hideMenu);
treeEl.addEventListener("dragover", (event) => {
  if (!state.dragPath || (event.target.closest && event.target.closest(".node"))) return;
  if (parentOf(state.dragPath) === ".") return;
  event.preventDefault();
  treeEl.classList.add("drop-target");
  event.dataTransfer.dropEffect = "move";
});
treeEl.addEventListener("dragleave", (event) => {
  if (!treeEl.contains(event.relatedTarget)) treeEl.classList.remove("drop-target");
});
treeEl.addEventListener("drop", (event) => {
  if (event.target.closest && event.target.closest(".node")) return;
  event.preventDefault();
  treeEl.classList.remove("drop-target");
  dropOnto(state.dragPath || event.dataTransfer.getData("text/plain"), ".");
});

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

function currentRunRange() {
  const selection = editor.state.selection.main;
  return runTarget(editor.state.doc.toString(), selection.head, selection.anchor);
}

function paintRunMarks() {
  editor.dispatch({
    effects: setRunStatuses.of(markersForRunStatus(editor.state.doc, runMarks)),
  });
}

function markStatement(from, status) {
  if (typeof from !== "number") return;
  const existing = runMarks.find((mark) => mark.from === from);
  if (existing) existing.status = status;
  else runMarks.push({ from, status });
  paintRunMarks();
}

function showProgress(done, total) {
  runProgress.hidden = false;
  if (typeof total === "number" && total > 0) {
    runProgress.max = total;
    runProgress.value = Math.min(Math.max(0, done || 0), total);
    resultMeta.textContent = runProgress.value + " / " + total;
  } else {
    runProgress.removeAttribute("value");
    runProgress.removeAttribute("max");
    resultMeta.textContent = "Running…";
  }
}

function hideProgress() {
  runProgress.hidden = true;
  runProgress.removeAttribute("value");
  runProgress.removeAttribute("max");
}

async function readNdjson(response, onEvent) {
  if (!response.ok) {
    const payload = await response.json().catch(() => ({}));
    throw new Error(payload.error || response.statusText);
  }
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  while (true) {
    const { value, done } = await reader.read();
    buffer += decoder.decode(value || new Uint8Array(), { stream: !done });
    let newline = buffer.indexOf("\n");
    while (newline >= 0) {
      const line = buffer.slice(0, newline).trim();
      buffer = buffer.slice(newline + 1);
      if (line) onEvent(JSON.parse(line));
      newline = buffer.indexOf("\n");
    }
    if (done) break;
  }
  if (buffer.trim()) onEvent(JSON.parse(buffer.trim()));
}

async function runSelection() {
  await flushSave();
  const range = currentRunRange();
  const dql = range ? editor.state.sliceDoc(range.from, range.to) : "";
  if (!dql.trim()) {
    resultMeta.textContent = "Nothing to run";
    resultBody.replaceChildren();
    hideProgress();
    return;
  }
  const origins = statementSpans(dql).map((span) => range.from + span.from);
  runMarks = origins.length ? [{ from: origins[0], status: "running" }] : [];
  paintRunMarks();
  runButton.disabled = true;
  resultMeta.textContent = "Running…";
  resultBody.replaceChildren();
  showProgress(null, null);
  const results = [];
  try {
    const response = await fetch("/api/run", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ dql }),
    });
    await readNdjson(response, (event) => {
      if (event.event === "error") throw new Error(event.error || "failed");
      if (event.event === "statement") {
        markStatement(origins[event.index], "running");
        showProgress(null, null);
        return;
      }
      if (event.event === "progress") {
        showProgress(event.done, event.total);
        return;
      }
      if (event.event === "result") {
        const result = event.result || {};
        results.push(result);
        markStatement(origins[event.index], result.ok ? "ok" : "error");
      }
    });
    renderResults(results);
  } catch (error) {
    resultMeta.textContent = "";
    resultBody.append(note(error.message, true));
    const running = runMarks.find((mark) => mark.status === "running");
    if (running) {
      running.status = "error";
      paintRunMarks();
    }
  } finally {
    hideProgress();
    runButton.disabled = false;
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
