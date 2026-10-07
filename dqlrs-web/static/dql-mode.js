/**
 * CodeMirror stream language for DQL.
 * Keyword coverage lives in dql-tokens.js; add words there.
 */
import { StreamLanguage } from "@codemirror/language";
import { Decoration, ViewPlugin } from "@codemirror/view";
import { queryBands, startState, tokenDql } from "./dql-tokens.js";

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

function bandDecorations(doc) {
  const ranges = [];
  for (const band of queryBands(doc.toString())) {
    if (band.band === "plain") continue;
    const deco = band.band === "write-even" ? writeEvenLine : band.band === "write" ? writeLine : altLine;
    const fromLine = doc.lineAt(band.from).number;
    const toLine = doc.lineAt(Math.max(band.from, band.to - 1)).number;
    for (let number = fromLine; number <= toLine; number += 1) {
      ranges.push(deco.range(doc.line(number).from));
    }
  }
  return Decoration.set(ranges, true);
}

/** Light stripe on even reads. Writes are red, and even writes are a darker red. */
export const dqlQueryBands = ViewPlugin.fromClass(
  class {
    constructor(view) {
      this.decorations = bandDecorations(view.state.doc);
    }

    update(update) {
      if (update.docChanged) this.decorations = bandDecorations(update.state.doc);
    }
  },
  { decorations: (plugin) => plugin.decorations },
);
