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
import { bandLayer, dqlLanguage, markersForRunStatus, queryHighlight, runButtonLayer, runFrame, runStatusField, runStatusGutter, setRunRequest, setRunStatuses } from "./dql-mode.js";
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
      runButtonLayer,
      bandLayer,
      syntaxHighlighting(dqlHighlight),
      placeholder("Open a .dql file, or create one."),
      EditorView.lineWrapping,
      EditorView.contentAttributes.of({ spellcheck: "false", id: "editor-content" }),
      drawSelection(),
      Prec.highest(EditorView.theme({
        ".cm-selectionBackground, &.cm-focused .cm-selectionLayer .cm-selectionBackground": {
          backgroundColor: "rgba(37, 99, 235, 0.28) !important",
        },
        ".cm-run-fill": {
          backgroundColor: "rgba(16, 42, 96, 0.05)",
          border: "1px solid #2563eb",
          borderRadius: "2px",
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
        ".cm-content": { padding: "1lh 0 5lh" },
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
const resultsEl = document.querySelector("#results");
const resultsResize = document.querySelector("#results-resize");
const RESULTS_HEIGHT_KEY = "dqlrs-web.results-height";
const EXPLAIN_JSON_KEY = "dqlrs-web.explain-json";

function resultsLimits() {
  const view = resultsEl.parentElement.getBoundingClientRect().height;
  const head = resultsEl.parentElement.querySelector(":scope > .pane-head");
  const headHeight = head ? head.getBoundingClientRect().height : 0;
  const min = 120;
  const max = Math.max(min, Math.floor(view - headHeight - 160));
  return { min, max };
}

function applyResultsHeight(height, persist) {
  const { min, max } = resultsLimits();
  const next = Math.round(Math.min(max, Math.max(min, height)));
  resultsEl.style.height = next + "px";
  resultsResize.setAttribute("aria-valuemin", String(min));
  resultsResize.setAttribute("aria-valuemax", String(max));
  resultsResize.setAttribute("aria-valuenow", String(next));
  if (persist) localStorage.setItem(RESULTS_HEIGHT_KEY, String(next));
  return next;
}

function restoreResultsHeight() {
  const saved = Number(localStorage.getItem(RESULTS_HEIGHT_KEY));
  if (!Number.isFinite(saved) || saved <= 0) {
    const { min, max } = resultsLimits();
    resultsResize.setAttribute("aria-valuemin", String(min));
    resultsResize.setAttribute("aria-valuemax", String(max));
    resultsResize.setAttribute("aria-valuenow", String(Math.round(resultsEl.getBoundingClientRect().height)));
    return;
  }
  applyResultsHeight(saved, false);
}

let resultsDrag = null;

resultsResize.addEventListener("pointerdown", (event) => {
  if (event.button !== 0) return;
  resultsResize.setPointerCapture(event.pointerId);
  resultsDrag = {
    y: event.clientY,
    height: resultsEl.getBoundingClientRect().height,
  };
  document.body.classList.add("results-resizing");
});

resultsResize.addEventListener("pointermove", (event) => {
  if (!resultsDrag) return;
  applyResultsHeight(resultsDrag.height - (event.clientY - resultsDrag.y), false);
});

function endResultsDrag(event) {
  if (!resultsDrag) return;
  if (event && resultsResize.hasPointerCapture(event.pointerId)) {
    resultsResize.releasePointerCapture(event.pointerId);
  }
  resultsDrag = null;
  document.body.classList.remove("results-resizing");
  applyResultsHeight(resultsEl.getBoundingClientRect().height, true);
}

resultsResize.addEventListener("pointerup", endResultsDrag);
resultsResize.addEventListener("pointercancel", endResultsDrag);

resultsResize.addEventListener("dblclick", () => {
  resultsEl.style.height = "";
  localStorage.removeItem(RESULTS_HEIGHT_KEY);
  restoreResultsHeight();
});

resultsResize.addEventListener("keydown", (event) => {
  if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
  event.preventDefault();
  const step = event.shiftKey ? 80 : 28;
  const current = resultsEl.getBoundingClientRect().height;
  applyResultsHeight(current + (event.key === "ArrowUp" ? step : -step), true);
});

requestAnimationFrame(restoreResultsHeight);
window.addEventListener("resize", () => {
  if (resultsEl.offsetParent === null || !resultsEl.style.height) return;
  applyResultsHeight(parseFloat(resultsEl.style.height), false);
});

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

function setRunBusy(busy) {
  runButton.disabled = busy;
  const floating = editor.dom.querySelector(".run-float");
  if (floating) floating.disabled = busy;
}

function runFromEditor() {
  if (runButton.disabled) return true;
  runSelection();
  return true;
}

setRunRequest(runFromEditor);
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
  const tables = mode === "tables";
  document.querySelector("#query-view").classList.toggle("hidden", mode !== "query");
  document.querySelector("#tables-view").classList.toggle("hidden", mode !== "tables");
  document.querySelector("#mode-query").classList.toggle("active", mode === "query");
  document.querySelector("#mode-tables").classList.toggle("active", mode === "tables");
  document.querySelector("#mode-query").setAttribute("aria-selected", mode === "query");
  document.querySelector("#mode-tables").setAttribute("aria-selected", mode === "tables");
  document.querySelector("#tree").classList.toggle("hidden", tables);
  document.querySelector("#tables-nav").classList.toggle("hidden", !tables);
  document.querySelector("#new-file").classList.toggle("hidden", tables);
  document.querySelector("#side-title").textContent = tables ? "Tables" : "Files";
  if (tables) loadTables();
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
    button.id = nodeDomId(child.path);
    button.dataset.testid = button.id;
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

function nodeDomId(path) {
  return "node-" + String(path).replaceAll("/", "__");
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
  input.id = "rename-input";
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

const menuIds = {
  "New file": "menu-new-file",
  "New folder": "menu-new-folder",
  Duplicate: "menu-duplicate",
  Rename: "menu-rename",
  "Move to…": "menu-move",
  Delete: "menu-delete",
};

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
    button.id = menuIds[item.label];
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
  if (state.mode !== "query" || event.target.closest(".node")) return;
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
  setRunBusy(true);
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
    setRunBusy(false);
  }
}

function renderResults(results) {
  resultBody.replaceChildren();
  const lastItems = [...results].reverse().find((item) => item.kind === "items" && item.ok);
  const notes = results.filter((item) => item !== lastItems);
  let explainSteps = 0;
  let otherNotes = 0;
  for (const item of notes) {
    const plan = item.ok && item.kind === "schema" ? parseExplainPlan(item.message || "") : null;
    if (plan) {
      explainSteps += plan.length;
      resultBody.append(explainPlan(plan));
      continue;
    }
    otherNotes += 1;
    const row = note(resultLabel(item), !item.ok);
    if (item.ok && item.kind === "affected") row.classList.add("ok");
    resultBody.append(row);
  }
  if (lastItems) {
    const rows = lastItems.items || [];
    resultMeta.textContent = rows.length + (rows.length === 1 ? " row" : " rows");
    resultBody.append(dataTable(rows, "result-table", lastItems.columns));
  } else if (explainSteps > 0 && otherNotes === 0) {
    resultMeta.textContent = explainSteps + (explainSteps === 1 ? " step" : " steps");
  } else {
    resultMeta.textContent = results.length ? "" : "Nothing to run";
  }
}

function parseExplainPlan(message) {
  const lines = message.split("\n").map((line) => line.trim()).filter(Boolean);
  if (!lines.length) return null;
  const steps = [];
  for (const line of lines) {
    const match = line.match(/^([a-z][a-z0-9_]*)\s+(\S+)(?:\s+(\{.*\}))?$/);
    if (!match) return null;
    const fields = match[3] ? parseExplainFields(match[3]) : [];
    if (!fields) return null;
    steps.push({ operation: match[1], target: match[2], fields });
  }
  return steps;
}

function parseExplainFields(text) {
  if (!text.startsWith("{") || !text.endsWith("}")) return null;
  const fields = [];
  let index = 1;
  const end = text.length - 1;
  while (index < end) {
    while (index < end && (text[index] === " " || text[index] === ",")) index += 1;
    if (index >= end) break;
    if (text[index] !== "'") return null;
    index += 1;
    let key = "";
    while (index < end && text[index] !== "'") {
      key += text[index];
      index += 1;
    }
    if (text[index] !== "'") return null;
    index += 1;
    if (text.slice(index, index + 2) !== ": ") return null;
    index += 2;
    if (text[index] !== '"') return null;
    index += 1;
    let value = "";
    while (index < end) {
      if (text[index] === "\\") {
        const next = text[index + 1] || "";
        value += next === "n" ? "\n" : next === "r" ? "\r" : next === "t" ? "\t" : next;
        index += 2;
        continue;
      }
      if (text[index] === '"') {
        index += 1;
        break;
      }
      value += text[index];
      index += 1;
    }
    fields.push({ key, value });
  }
  return fields;
}

function explainPlan(steps) {
  const card = document.createElement("section");
  card.className = "explain";
  card.dataset.testid = "explain-plan";
  const showJson = localStorage.getItem(EXPLAIN_JSON_KEY) === "1";
  if (showJson) card.classList.add("raw");
  const head = document.createElement("div");
  head.className = "explain-head";
  const title = document.createElement("span");
  title.textContent = "Explain";
  const toggle = document.createElement("label");
  toggle.className = "explain-raw";
  const input = document.createElement("input");
  input.type = "checkbox";
  input.dataset.testid = "explain-raw";
  input.checked = showJson;
  input.setAttribute("aria-label", "Show raw JSON");
  const caption = document.createElement("span");
  caption.textContent = "JSON";
  toggle.append(input, caption);
  head.append(title, toggle);
  input.addEventListener("change", () => {
    card.classList.toggle("raw", input.checked);
    localStorage.setItem(EXPLAIN_JSON_KEY, input.checked ? "1" : "0");
  });
  const json = document.createElement("pre");
  json.className = "explain-json";
  json.dataset.testid = "explain-json";
  json.textContent = JSON.stringify(steps.map((step) => {
    const row = { operation: step.operation, target: step.target };
    for (const field of step.fields) row[field.key] = field.value;
    return row;
  }), null, 2);
  card.append(head, json);
  for (const step of steps) {
    const row = document.createElement("div");
    row.className = "explain-step";
    row.dataset.testid = "explain-step";
    const op = document.createElement("div");
    op.className = "explain-op";
    const badge = document.createElement("span");
    badge.className = "explain-badge";
    badge.dataset.testid = "explain-op";
    badge.textContent = step.operation.replaceAll("_", " ");
    const target = document.createElement("span");
    target.className = "explain-target";
    target.dataset.testid = "explain-target";
    target.textContent = step.target;
    op.append(badge, target);
    row.append(op);
    if (step.fields.length) {
      const list = document.createElement("dl");
      list.className = "explain-fields";
      for (const field of step.fields) {
        const term = document.createElement("dt");
        term.textContent = field.key.replaceAll("_", " ");
        const detail = document.createElement("dd");
        detail.textContent = field.value;
        list.append(term, detail);
      }
      row.append(list);
    }
    card.append(row);
  }
  return card;
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
  div.dataset.testid = "result-note";
  div.textContent = text;
  return div;
}

function dataTable(rows, tableId, columns) {
  if (!rows.length) return note("No rows", false);
  const keys = tableColumns(rows, columns);
  const table = document.createElement("table");
  if (tableId) table.id = tableId;
  const head = document.createElement("tr");
  for (const column of keys) {
    const cell = document.createElement("th");
    paintKeyColumn(cell, column);
    const label = document.createElement("span");
    label.className = "col-label";
    const name = document.createElement("span");
    name.textContent = column.name;
    label.append(name);
    for (const icon of keyIcons(column)) label.append(icon);
    cell.append(label);
    head.append(cell);
  }
  const thead = document.createElement("thead");
  thead.append(head);
  const body = document.createElement("tbody");
  for (const row of rows) {
    const tr = document.createElement("tr");
    for (const column of keys) {
      const cell = document.createElement("td");
      paintKeyColumn(cell, column);
      const value = row[column.name];
      cell.textContent = value === null || value === undefined ? "" : typeof value === "object" ? JSON.stringify(value) : String(value);
      tr.append(cell);
    }
    body.append(tr);
  }
  table.append(thead, body);
  return table;
}

function paintKeyColumn(cell, column) {
  const tableKey = column.table || "";
  const indexKey = column.index || "";
  if (tableKey && indexKey) cell.classList.add("col-both");
  else if (tableKey) cell.classList.add("col-table");
  else if (indexKey) cell.classList.add("col-index");
}

function keyIcons(column) {
  const icons = [];
  const hash = column.table === "hash" || column.index === "hash";
  const range = column.table === "range" || column.index === "range";
  if (hash) icons.push(keyIcon("hash", keyTitle(column, "hash")));
  if (range) icons.push(keyIcon("range", keyTitle(column, "range")));
  return icons;
}

function keyTitle(column, role) {
  const parts = [];
  if (column.table === role) parts.push(role === "hash" ? "Table hash key" : "Table range key");
  if (column.index === role) parts.push(role === "hash" ? "Index hash key" : "Index range key");
  return parts.join(", ");
}

function tableKeyLine(keys) {
  const line = document.createElement("span");
  line.className = "keys";
  const listed = Array.isArray(keys) ? keys : [];
  for (const key of listed) {
    if (!key || !key.name) continue;
    if (line.childNodes.length) {
      const sep = document.createElement("span");
      sep.className = "key-sep";
      sep.textContent = "·";
      line.append(sep);
    }
    const pair = document.createElement("span");
    pair.className = "key-pair";
    if (key.role === "hash" || key.role === "range") {
      pair.append(keyIcon(key.role, key.role === "hash" ? "Hash key" : "Range key"));
    }
    const name = document.createElement("span");
    name.textContent = key.name;
    pair.append(name);
    line.append(pair);
  }
  return line;
}

function keyIcon(role, title) {
  const icon = document.createElement("span");
  icon.className = "key-icon";
  icon.dataset.key = role;
  icon.title = title;
  icon.setAttribute("role", "img");
  icon.setAttribute("aria-label", title);
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 12 12");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("fill", "none");
  path.setAttribute("stroke", "currentColor");
  path.setAttribute("stroke-width", "1.3");
  path.setAttribute("stroke-linecap", "round");
  path.setAttribute("stroke-linejoin", "round");
  path.setAttribute(
    "d",
    role === "hash"
      ? "M4.2 1.2v9.6M7.8 1.2v9.6M1.3 4.2h9.4M1.3 7.8h9.4"
      : "M6 1.3v9.4M6 1.3 3.7 3.6M6 1.3l2.3 2.3M6 10.7 3.7 8.4M6 10.7l2.3-2.3",
  );
  svg.append(path);
  icon.append(svg);
  return icon;
}

function tableColumns(rows, columns) {
  const present = [];
  for (const row of rows) {
    for (const key of Object.keys(row)) {
      if (!present.includes(key)) present.push(key);
    }
  }
  const described = new Map();
  if (Array.isArray(columns)) {
    for (const column of columns) {
      const name = typeof column === "string" ? column : column && column.name;
      if (name) described.set(name, typeof column === "string" ? { name } : column);
    }
  }
  const ordered = [];
  for (const name of described.keys()) {
    if (present.includes(name) && !ordered.includes(name)) ordered.push(name);
  }
  if (!ordered.length) {
    return present.map((name) => ({ name }));
  }
  const extra = present.filter((key) => !ordered.includes(key));
  extra.sort((left, right) => left.localeCompare(right));
  return ordered.concat(extra).map((name) => described.get(name) || { name });
}

async function loadTables() {
  const pattern = document.querySelector("#table-search").value.trim();
  const list = document.querySelector("#table-list");
  list.replaceChildren(note("Loading…", false));
  try {
    const payload = await api("/api/tables?pattern=" + encodeURIComponent(pattern));
    list.replaceChildren();
    if (!payload.tables.length) {
      state.table = "";
      list.append(note(pattern ? "No tables match " + pattern : "No tables", false));
      clearRows();
      return;
    }
    if (!payload.tables.some((table) => table.name === state.table)) {
      state.table = payload.tables[0].name;
      state.page = 0;
    }
    for (const table of payload.tables) {
      const button = document.createElement("button");
      button.type = "button";
      button.id = "table-" + table.name.replaceAll(/[^A-Za-z0-9_-]/g, "_");
      button.className = "table-row" + (table.name === state.table ? " selected" : "");
      const name = document.createElement("strong");
      name.textContent = table.name;
      button.append(name, tableKeyLine(table.keys));
      button.addEventListener("click", () => selectTable(table.name));
      list.append(button);
    }
    await loadRows();
  } catch (error) {
    list.replaceChildren(note(error.message, true));
  }
}

function selectTable(name) {
  if (state.table === name && state.page === 0) return;
  state.table = name;
  state.page = 0;
  for (const button of document.querySelectorAll("#table-list .table-row")) {
    button.classList.toggle("selected", button.id === "table-" + name.replaceAll(/[^A-Za-z0-9_-]/g, "_"));
  }
  loadRows();
}

function clearRows() {
  document.querySelector("#rows-title").textContent = "";
  document.querySelector("#rows-meta").textContent = "";
  document.querySelector("#page-label").textContent = "";
  document.querySelector("#page-prev").disabled = true;
  document.querySelector("#page-next").disabled = true;
  document.querySelector("#rows-body").replaceChildren();
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
    body.replaceChildren(dataTable(payload.items || [], "rows-table", payload.columns));
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
