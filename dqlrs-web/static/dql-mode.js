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

function boundsOf(rects) {
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
  return new RectangleMarker("cm-run-fill", left, top, Math.max(0, right - left), Math.max(0, bottom - top));
}

/** One background for the query or selection that Run will execute. */
function runFillMarkers(view) {
  const selection = view.state.selection.main;
  const target = runTarget(view.state.doc.toString(), selection.head, selection.anchor);
  if (!target || target.from >= target.to) return [];
  const rects = RectangleMarker.forRange(view, "cm-run-fill", {
    from: target.from,
    to: target.to,
    empty: false,
  });
  if (rects.length <= 1) return rects;
  return [boundsOf(rects)];
}

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

