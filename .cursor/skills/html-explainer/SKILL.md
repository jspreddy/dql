---
name: html-explainer
description: Explain a topic hierarchically, with sections and subsections using Mermaid diagrams, by writing an HTML file. Use when the user asks for an HTML explanation, a hierarchical write-up, or a sectioned diagram page. Follow the mermaid-diagrams skill.
---

# HTML explainer

Explain something hierarchically, with sections and subsections using mermaid diagrams, by writing a html file. Use the mermaid-diagrams skill.

Read and follow `.cursor/skills/mermaid-diagrams/SKILL.md` before writing diagrams. Do not hand-build SVG.

## Page

Write one self-contained HTML file. Match the paper page used by `justfile.html` and `rust-docs/architecture.html`:

- Background `#f3efe6`, paper `#fffdf8`, ink `#1c1915`, muted `#5e564c`, line `#d9cdb8`, links `#9a3412`.
- Body font `"Iowan Old Style", Palatino, Georgia, serif`. Kicker, nav, and diagram captions use `ui-sans-serif`.
- Header with a short kicker, one `h1`, and one sentence on what the page explains.
- A nav of links to each section.
- Each diagram sits in a `<figure>` with an `h2` or `h3` in the `figcaption` and one sentence under it.

## Hierarchy

Lead with the whole subject, then narrow.

1. One overview section. Its diagram is the top of the hierarchy: groups, or the main parts and how they connect.
2. One section per part. Each section has its own diagram of that part only.
3. Subsections (`h3`) when a part has internal structure. Each subsection has its own diagram of that slice.

Prose under a diagram names what the picture does not: order, conditions, and which box is which. Do not repeat every node in a paragraph.

Choose the Mermaid type that fits the relationship:

- Containment: `mindmap`.
- Calls or flow: `flowchart`. Add the call-chain hover and click-to-pin from the mermaid-diagrams skill.
- Quantities moving between stages: `sankey-beta`.

A flowchart that is a call graph gets the highlight script. A mind map or Sankey does not.

## Check

Open the file over `http://`, not `file://`. Confirm every section diagram renders, and on a call graph hover a node, click to pin it, move the pointer away, then click again to clear.
