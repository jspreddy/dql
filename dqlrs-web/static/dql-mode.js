/**
 * CodeMirror stream language for DQL.
 * Keyword coverage lives in dql-tokens.js; add words there.
 */
import { StreamLanguage } from "@codemirror/language";
import { Facet, RangeSet, RangeSetBuilder, StateEffect, StateField } from "@codemirror/state";
import { Decoration, GutterMarker, RectangleMarker, ViewPlugin, gutter, layer } from "@codemirror/view";
import { bandAppearance, queryBands, runTarget, startState, tokenDql } from "./dql-tokens.js";

export const dqlLanguage = StreamLanguage.define({
  name: "dql",
  startState,
  token: tokenDql,
  languageData: {
    commentTokens: { line: "--" },
  },
});

const altLine = Decoration.line({ class: "cm-dql-alt" });
const writeLine = Decoration.line({ class: "cm-dql-write" });
const writeEvenLine = Decoration.line({ class: "cm-dql-write-even" });
const lineDecoration = {
  alt: altLine,
  write: writeLine,
  "write-even": writeEvenLine,
};

class WriteBar extends GutterMarker {
  constructor(even) {
    super();
    this.even = even;
  }

  eq(other) {
    return other instanceof WriteBar && other.even === this.even;
  }

  toDOM() {
    const mark = document.createElement("span");
    mark.className = "cm-write-bar" + (this.even ? " cm-write-bar-even" : "");
    return mark;
  }
}

const writeBar = new WriteBar(false);
const writeBarEven = new WriteBar(true);

export const highlightOptions = Facet.define({
  combine(values) {
    return values.length ? values[values.length - 1] : { minimalWrite: false, evenOdd: true };
  },
});

function paint(doc, options) {
  const ranges = [];
  const markers = new RangeSetBuilder();
  for (const band of queryBands(doc.toString())) {
    const appearance = bandAppearance(band, options);
    const deco = lineDecoration[appearance];
    const marker = appearance === "bar-even" ? writeBarEven : appearance === "bar" ? writeBar : null;
    for (let number = band.lineFrom; number <= band.lineTo; number += 1) {
      const line = doc.line(number);
      if (deco) ranges.push(deco.range(line.from));
      if (marker) markers.add(line.from, line.from, marker);
    }
  }
  return { decorations: Decoration.set(ranges, true), markers: markers.finish() };
}

/** Line bands, or red gutter bars when minimal write highlight is on. */
export const dqlQueryBands = ViewPlugin.fromClass(
  class {
    constructor(view) {
      const painted = paint(view.state.doc, view.state.facet(highlightOptions));
      this.decorations = painted.decorations;
      this.markers = painted.markers;
    }

    update(update) {
      const previous = update.startState.facet(highlightOptions);
      const next = update.state.facet(highlightOptions);
      if (update.docChanged || previous.minimalWrite !== next.minimalWrite || previous.evenOdd !== next.evenOdd) {
        const painted = paint(update.state.doc, next);
        this.decorations = painted.decorations;
        this.markers = painted.markers;
      }
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

const writeGutter = gutter({
  class: "cm-dql-gutter",
  markers(view) {
    return view.plugin(dqlQueryBands)?.markers ?? RangeSet.empty;
  },
  initialSpacer: () => writeBar,
});

export function queryHighlight(options) {
  const extensions = [highlightOptions.of(options), dqlQueryBands];
  if (options.minimalWrite) extensions.push(writeGutter);
  return extensions;
}

const bandClass = {
  alt: "cm-band-alt",
  write: "cm-band-write",
  "write-even": "cm-band-write-even",
};

function layerOrigin(view) {
  const rect = view.scrollDOM.getBoundingClientRect();
  return {
    left: rect.left - view.scrollDOM.scrollLeft * view.scaleX,
    top: rect.top - view.scrollDOM.scrollTop * view.scaleY,
  };
}

/** Even/odd and write colors, behind the run-range fill so that fill stays visible. */
function bandMarkers(view) {
  const options = view.state.facet(highlightOptions);
  const origin = layerOrigin(view);
  const content = view.contentDOM.getBoundingClientRect();
  const left = content.left - origin.left;
  const width = Math.max(0, content.right - content.left);
  const docTop = view.documentTop;
  const doc = view.state.doc;
  const markers = [];
  for (const band of queryBands(doc.toString())) {
    const className = bandClass[bandAppearance(band, options)];
    if (!className) continue;
    for (let number = band.lineFrom; number <= band.lineTo; number += 1) {
      const line = doc.line(number);
      if (line.to < view.viewport.from || line.from > view.viewport.to) continue;
      const block = view.lineBlockAt(line.from);
      markers.push(
        new RectangleMarker(
          className,
          left,
          docTop + block.top - origin.top,
          width,
          Math.max(0, block.bottom - block.top),
        ),
      );
    }
  }
  return markers;
}

export const bandLayer = layer({
  above: false,
  class: "cm-band-layer",
  markers: bandMarkers,
  update(update) {
    const previous = update.startState.facet(highlightOptions);
    const next = update.state.facet(highlightOptions);
    return (
      update.docChanged ||
      update.viewportChanged ||
      previous.minimalWrite !== next.minimalWrite ||
      previous.evenOdd !== next.evenOdd
    );
  },
});

const runGap = 5;

function tightBounds(rects) {
  let left = Infinity;
  let top = Infinity;
  let right = -Infinity;
  let bottom = -Infinity;
  for (const rect of rects) {
    const width = rect.width == null ? 0 : rect.width;
    left = Math.min(left, rect.left);
    top = Math.min(top, rect.top);
    right = Math.max(right, rect.left + width);
    bottom = Math.max(bottom, rect.top + rect.height);
  }
  return { left, top, width: Math.max(0, right - left), height: Math.max(0, bottom - top) };
}

/** Grow the box so the 1px border's inner edge sits 5px out from the text. */
function withGap(box) {
  const border = 1;
  return new RectangleMarker(
    "cm-run-fill",
    box.left - runGap - border,
    box.top - runGap - border,
    box.width + runGap * 2,
    box.height + runGap * 2,
  );
}

/**
 * A query band ends at the first character of the following line. A rectangle
 * drawn at that position is as tall as the next line, so the box would cover
 * it. Stop on the previous line instead. A range that ends at the end of the
 * document stays put, so a trailing blank line at the end of the file remains
 * inside the box.
 */
function paintEnd(doc, from, to) {
  if (to > from && to < doc.length && doc.lineAt(to).from === to) return to - 1;
  return to;
}

/**
 * Range the query selection window frames.
 * A text selection grows to each line it touches, so a partial line is framed
 * as a whole line. A cursor inside a query keeps that query's own range.
 */
function windowRange(doc, target) {
  if (target.kind !== "selection") return { from: target.from, to: target.to };
  const start = doc.lineAt(Math.min(target.from, doc.length));
  const last = Math.min(Math.max(target.from, target.to - 1), doc.length);
  const end = doc.lineAt(last);
  return { from: start.from, to: end.to };
}

/** Blue window around the query, or around every line a text selection touches. */
export function runBox(view) {
  const selection = view.state.selection.main;
  const target = runTarget(view.state.doc.toString(), selection.head, selection.anchor);
  if (!target || target.from >= target.to) return null;
  const range = windowRange(view.state.doc, target);
  const to = paintEnd(view.state.doc, range.from, range.to);
  if (range.from >= to) return null;
  const rects = RectangleMarker.forRange(view, "cm-run-fill", {
    from: range.from,
    to,
    empty: false,
  });
  if (!rects.length) return null;
  return withGap(tightBounds(rects));
}

function runFillMarkers(view) {
  const box = runBox(view);
  return box ? [box] : [];
}

const runButtonSize = 22;
const runButtonGap = 6;

let requestRun = () => {};

/** Called when the floating play button asks to run the outlined range. */
export function setRunRequest(fn) {
  requestRun = fn;
}

function runButtonOrigin(box, view) {
  let left = box.left - runButtonGap - runButtonSize;
  const minLeft = view.scrollDOM.scrollLeft + 2;
  if (left < minLeft) left = minLeft;
  let top = box.top + (box.height - runButtonSize) / 2;
  const visibleTop = view.scrollDOM.scrollTop;
  const visibleBottom = visibleTop + view.scrollDOM.clientHeight;
  const minTop = Math.max(box.top - 2, visibleTop + 4);
  const maxTop = Math.min(box.top + Math.max(0, box.height - runButtonSize) + 2, visibleBottom - runButtonSize - 4);
  if (maxTop >= minTop) top = Math.min(Math.max(top, minTop), maxTop);
  return { left, top };
}

class RunButtonMarker {
  constructor(left, top, disabled) {
    this.left = left;
    this.top = top;
    this.disabled = disabled;
  }

  eq(other) {
    return other instanceof RunButtonMarker && other.left === this.left && other.top === this.top && other.disabled === this.disabled;
  }

  draw() {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "run-float";
    button.title = "Run selection";
    button.setAttribute("aria-label", "Run selection");
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", "0 0 12 12");
    svg.setAttribute("aria-hidden", "true");
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("fill", "currentColor");
    path.setAttribute("d", "M4.1 2.15 9.7 6 4.1 9.85z");
    svg.append(path);
    button.append(svg);
    const hold = (event) => {
      event.preventDefault();
      event.stopPropagation();
    };
    button.addEventListener("pointerdown", hold);
    button.addEventListener("mousedown", hold);
    button.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      if (!button.disabled) requestRun();
    });
    this.adjust(button);
    return button;
  }

  update(elt, prev) {
    if (!(prev instanceof RunButtonMarker)) return false;
    this.adjust(elt);
    return true;
  }

  adjust(elt) {
    elt.style.left = this.left + "px";
    elt.style.top = this.top + "px";
    elt.disabled = this.disabled;
  }
}

function runButtonMarkers(view) {
  const box = runBox(view);
  if (!box) return [];
  const origin = runButtonOrigin(box, view);
  const busy = document.querySelector("#run")?.disabled === true;
  return [new RunButtonMarker(origin.left, origin.top, busy)];
}

/** Play button sitting just left of the query selection window. */
export const runButtonLayer = layer({
  above: true,
  class: "cm-run-button-layer",
  markers: runButtonMarkers,
  update(update) {
    return update.docChanged || update.selectionSet || update.viewportChanged || update.geometryChanged;
  },
  mount(dom) {
    dom.removeAttribute("aria-hidden");
  },
});

export const runFrame = layer({
  above: false,
  class: "cm-run-fill-layer",
  markers: runFillMarkers,
  update(update) {
    return update.docChanged || update.selectionSet || update.viewportChanged;
  },
});

class RunStatusMarker extends GutterMarker {
  constructor(status) {
    super();
    this.status = status;
  }

  eq(other) {
    return other instanceof RunStatusMarker && other.status === this.status;
  }

  toDOM() {
    const mark = document.createElement("span");
    mark.className = "run-status run-status-" + this.status;
    const label = this.status === "running" ? "Running" : this.status === "ok" ? "Succeeded" : "Failed";
    mark.title = label;
    mark.setAttribute("role", "img");
    mark.setAttribute("aria-label", label);
    mark.dataset.testid = "run-status-" + this.status;
    if (this.status === "ok") mark.textContent = "✓";
    if (this.status === "error") mark.textContent = "×";
    return mark;
  }
}

const statusMarker = {
  running: new RunStatusMarker("running"),
  ok: new RunStatusMarker("ok"),
  error: new RunStatusMarker("error"),
};

/** Replace the gutter marks for the queries in the latest run. */
export const setRunStatuses = StateEffect.define();

export const runStatusField = StateField.define({
  create() {
    return RangeSet.empty;
  },
  update(markers, transaction) {
    for (const effect of transaction.effects) {
      if (effect.is(setRunStatuses)) return effect.value;
    }
    return markers.map(transaction.changes);
  },
});

export function markersForRunStatus(doc, marks) {
  const byLine = new Map();
  for (const mark of marks) {
    if (typeof mark.from !== "number" || mark.from < 0) continue;
    const marker = statusMarker[mark.status];
    if (!marker) continue;
    const line = doc.lineAt(Math.min(mark.from, doc.length));
    byLine.set(line.from, marker);
  }
  const builder = new RangeSetBuilder();
  for (const from of [...byLine.keys()].sort((left, right) => left - right)) {
    builder.add(from, from, byLine.get(from));
  }
  return builder.finish();
}

/** Blue spinner, green check, or red cross on the query that ran. */
export const runStatusGutter = gutter({
  class: "cm-run-status-gutter",
  markers(view) {
    return view.state.field(runStatusField);
  },
});

