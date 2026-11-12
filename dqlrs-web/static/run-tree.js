// The run dialog's file tree, per-file dots and summaries, and totals tiles.
//
// A run file is { doc: { path }, statements, counts, status, error, row }. status is one of
// waiting, running, ok, error, skipped. buildRunTree sets file.row; paintRunFile repaints it.

import { amountPhrase, fileSummaryText, formatUnits } from "./statement-text.js";

function defaultTip(el, text) {
  if (text) el.dataset.tip = text;
  else delete el.dataset.tip;
}

/**
 * Files as a tree of folders, relative to the folder that was run. Each folder lists its own
 * files before its subfolders, the order the files run in.
 */
export function buildRunTree(path, kind, files, { tip = defaultTip } = {}) {
  if (!files.length) {
    const empty = document.createElement("p");
    empty.className = "empty";
    empty.textContent = "This folder has no .dql files.";
    return empty;
  }
  const base = kind === "dir" && path !== "." ? path + "/" : "";
  const root = { dirs: new Map(), files: [] };
  for (const file of files) {
    const relative = kind === "dir" ? file.doc.path.slice(base.length) : file.doc.path.split("/").pop();
    const parts = relative.split("/");
    let node = root;
    for (const part of parts.slice(0, -1)) {
      if (!node.dirs.has(part)) node.dirs.set(part, { dirs: new Map(), files: [] });
      node = node.dirs.get(part);
    }
    node.files.push({ file, name: parts[parts.length - 1] });
  }
  const list = document.createElement("ul");
  list.className = "run-tree";
  list.dataset.testid = "run-tree";
  appendRunTree(list, root, tip);
  return list;
}

function appendRunTree(list, node, tip) {
  for (const { file, name } of node.files) {
    const item = document.createElement("li");
    item.className = "run-tree-file";
    const row = document.createElement("div");
    row.className = "run-tree-row";
    row.dataset.path = file.doc.path;
    row.dataset.testid = "run-file";
    const dot = document.createElement("span");
    dot.className = "run-dot";
    dot.dataset.testid = "run-dot";
    const label = document.createElement("span");
    label.className = "run-tree-name";
    label.textContent = name;
    const count = document.createElement("span");
    count.className = "run-tree-count";
    count.textContent = file.statements ? amountPhrase(file.statements, "statement", "statements") : "no statements";
    const summary = document.createElement("span");
    summary.className = "run-tree-summary";
    summary.dataset.testid = "run-summary";
    row.append(dot, label, count, summary);
    item.append(row);
    list.append(item);
    file.row = row;
    paintRunFile(file, { tip });
  }
  for (const [name, child] of [...node.dirs.entries()].sort(([a], [b]) => a.localeCompare(b))) {
    const item = document.createElement("li");
    item.className = "run-tree-dir";
    const row = document.createElement("div");
    row.className = "run-tree-row";
    const label = document.createElement("span");
    label.className = "run-tree-name";
    label.textContent = name + "/";
    row.append(label);
    const nested = document.createElement("ul");
    appendRunTree(nested, child, tip);
    item.append(row, nested);
    list.append(item);
  }
}

/** Repaint a file's dot and summary from its status, counts, and error. */
export function paintRunFile(file, { tip = defaultTip } = {}) {
  if (!file.row) return;
  const dot = file.row.querySelector(".run-dot");
  const summary = file.row.querySelector(".run-tree-summary");
  dot.dataset.status = file.status;
  file.row.dataset.status = file.status;
  summary.classList.toggle("is-error", file.status === "error");
  if (file.status === "waiting") summary.textContent = "";
  else if (file.status === "skipped") summary.textContent = file.statements ? "Not run" : "Skipped";
  else if (file.status === "running") summary.textContent = file.counts.queries ? fileSummaryText(file.counts) : "Running…";
  else summary.textContent = file.error && !file.counts.queries ? file.error : fileSummaryText(file.counts);
  if (file.error) tip(summary, file.error);
}

/** Fill `container` with one tile per total. */
export function renderRunTotals(container, counts) {
  container.hidden = false;
  const tiles = [
    ["queries", "Queries", counts.queries],
    ["reads", "Reads", counts.reads],
    ["writes", "Writes", counts.writes],
    ["success", "Success", counts.success],
    ["failure", "Failure", counts.failure],
    ["read-units", "Read units", formatUnits(counts.readUnits)],
    ["write-units", "Write units", formatUnits(counts.writeUnits)],
  ];
  container.replaceChildren(...tiles.map(([key, label, value]) => {
    const tile = document.createElement("div");
    tile.className = "run-total";
    tile.dataset.total = key;
    tile.dataset.testid = "run-total-" + key;
    if (key === "failure" && !value) tile.dataset.zero = "";
    const name = document.createElement("span");
    name.className = "run-total-label";
    name.textContent = label;
    const amount = document.createElement("span");
    amount.className = "run-total-value";
    amount.textContent = String(value);
    tile.append(name, amount);
    return tile;
  }));
}
