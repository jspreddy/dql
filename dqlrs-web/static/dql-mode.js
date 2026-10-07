/**
 * CodeMirror stream language for DQL.
 * Keyword coverage lives in dql-tokens.js; add words there.
 */
import { StreamLanguage } from "@codemirror/language";
import { Facet, RangeSet, RangeSetBuilder } from "@codemirror/state";
import { Decoration, GutterMarker, ViewPlugin, gutter } from "@codemirror/view";
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

const runLine = Decoration.line({ class: "cm-run-line" });
const runTop = Decoration.line({ class: "cm-run-line cm-run-top" });
const runBottom = Decoration.line({ class: "cm-run-line cm-run-bottom" });
const runOnly = Decoration.line({ class: "cm-run-line cm-run-top cm-run-bottom" });
const runMark = Decoration.mark({ class: "cm-run-mark" });

function coversWholeLines(doc, from, to) {
  if (from >= to) return false;
  const start = doc.lineAt(from);
  const end = doc.lineAt(Math.min(to - 1, doc.length));
  return from === start.from && to >= end.to;
}

/** Border around the query or selection that Run will execute. */
export const runFrame = ViewPlugin.fromClass(
  class {
    constructor(view) {
      this.decorations = frame(view.state);
    }

    update(update) {
      if (update.docChanged || update.selectionSet) this.decorations = frame(update.state);
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

function frame(state) {
  const selection = state.selection.main;
  const target = runTarget(state.doc.toString(), selection.head, selection.anchor);
  if (!target) return Decoration.none;
  const doc = state.doc;
  if (!coversWholeLines(doc, target.from, target.to)) {
    return Decoration.set([runMark.range(target.from, target.to)]);
  }
  const start = doc.lineAt(target.from).number;
  const end = doc.lineAt(Math.min(Math.max(target.from, target.to - 1), doc.length)).number;
  const ranges = [];
  for (let number = start; number <= end; number += 1) {
    const deco = number === start && number === end ? runOnly : number === start ? runTop : number === end ? runBottom : runLine;
    ranges.push(deco.range(doc.line(number).from));
  }
  return Decoration.set(ranges);
}
