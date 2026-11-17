// The table browser's Keys box: a small CodeMirror editor, the same library as the query
// editor, with its own highlighting. Keys alternate two colors, every other one on a faint
// band; inside a tuple the range part has its own color; commas and the parentheses around a
// list of tuples stay quiet; what cannot be a key is underlined. The pieces come from
// keyHighlightSpans in table-query.js, which the unit tests cover.

import { Compartment, EditorState, Prec } from "@codemirror/state";
import { Decoration, EditorView, ViewPlugin, keymap, placeholder } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { keyHighlightSpans } from "./table-query.js";

function keyDecorations(text) {
  const ranges = [];
  let at = 0;
  let key = null;
  const closeKey = (end) => {
    if (key && end > key.from) {
      ranges.push(Decoration.mark({ class: `kh-key kh-alt-${key.index % 2}` }).range(key.from, end));
    }
    key = null;
  };
  let lastEnd = 0;
  for (const span of keyHighlightSpans(text)) {
    const from = at;
    const to = at + span.text.length;
    at = to;
    if (span.key === null) {
      if (key) closeKey(lastEnd);
    } else if (!key || key.index !== span.key) {
      if (key) closeKey(lastEnd);
      key = { index: span.key, from };
    }
    if (span.kind && to > from) {
      // The color class rides on each token, so it holds however CodeMirror nests the marks.
      const tone = span.key === null ? "" : ` kh-c${span.key % 2}`;
      ranges.push(Decoration.mark({ class: `kh-${span.kind}${tone}` }).range(from, to));
    }
    if (span.text.trim()) lastEnd = to;
  }
  if (key) closeKey(lastEnd);
  return Decoration.set(ranges, true);
}

const keyHighlighting = ViewPlugin.fromClass(
  class {
    constructor(view) {
      this.decorations = keyDecorations(view.state.doc.toString());
    }
    update(update) {
      if (update.docChanged) this.decorations = keyDecorations(update.state.doc.toString());
    }
  },
  { decorations: (plugin) => plugin.decorations },
);

/**
 * Replace `textarea` with the Keys editor and return the interface table-query.js uses:
 * `value`, `placeholder`, `focus()`, `hasFocus()`, `onInput(listener)`. `submit` runs on
 * Ctrl/Cmd+Enter, as in the query editor.
 */
export function createKeysEditor(textarea, { submit = () => {} } = {}) {
  const listeners = [];
  const placeholderSlot = new Compartment();
  let applying = false;
  const view = new EditorView({
    state: EditorState.create({
      doc: textarea.value,
      extensions: [
        history(),
        Prec.highest(keymap.of([{ key: "Mod-Enter", run: () => (submit(), true) }])),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        EditorView.lineWrapping,
        placeholderSlot.of(placeholder(textarea.placeholder || "")),
        keyHighlighting,
        // CodeMirror owns the root element's attributes; set the class and id through it.
        EditorView.editorAttributes.of({ class: "keys-editor", id: textarea.id }),
        EditorView.contentAttributes.of({
          "data-testid": textarea.dataset.testid || "table-query-batch-keys",
          "aria-label": textarea.getAttribute("aria-label") || "Primary keys",
          spellcheck: "false",
          autocorrect: "off",
          autocapitalize: "off",
        }),
        EditorView.updateListener.of((update) => {
          if (update.docChanged && !applying) for (const listener of listeners) listener();
        }),
      ],
    }),
  });
  textarea.replaceWith(view.dom);

  return {
    view,
    get value() {
      return view.state.doc.toString();
    },
    set value(text) {
      const next = String(text ?? "");
      if (next === view.state.doc.toString()) return;
      applying = true;
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } });
      applying = false;
    },
    set placeholder(text) {
      view.dispatch({ effects: placeholderSlot.reconfigure(placeholder(text || "")) });
    },
    focus: () => view.focus(),
    hasFocus: () => view.hasFocus,
    onInput: (listener) => listeners.push(listener),
  };
}
