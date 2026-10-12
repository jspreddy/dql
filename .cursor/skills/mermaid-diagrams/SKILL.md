---
name: mermaid-diagrams
description: Render diagrams with Mermaid from the jsDelivr CDN instead of hand-built SVG. Adds flowchart hover highlighting of the full caller and callee chain, and click-to-pin with a filled node. Use when adding or editing Mermaid diagrams, Sankey charts, mind maps, flowcharts, call graphs, or when the user asks for diagram hover or click highlighting.
---

# Mermaid diagrams

Do not hand-build SVG diagrams. Use Mermaid 12 from the jsDelivr CDN. Docs: https://mermaid.ai/open-source/intro/getting-started.html

## Page setup

Put each diagram in its own `<pre class="mermaid">`. Load Mermaid once, after the diagrams:

```html
<script type="module">
  import mermaid from 'https://cdn.jsdelivr.net/npm/mermaid@12/dist/mermaid.esm.min.mjs';
  mermaid.initialize({ startOnLoad: true, theme: 'neutral' });
</script>
```

```css
pre.mermaid { margin: 0; background: transparent; }
.mermaid svg { max-width: 100%; height: auto; }
```

If a script must run after render, set `startOnLoad: false`, copy any `<pre>` text you still need, then `await mermaid.run()`. Mermaid replaces the `<pre>` contents with the SVG.

Scope SVG rules to `.mermaid svg`. A global `svg { width: 100% }` stretches icons elsewhere on the page.

## Diagram text

- Flowchart node ids are camelCase. Hyphens are parsed as subtraction. Put the visible name in brackets: `webTest[web-test]`.
- A subgraph id must differ from every node id inside it. `subgraph rust` cannot contain `rust[rust]`; use `subgraph rustg [rust]`.
- Quote labels that contain `/` or other punctuation: `scrNote["notebook/start.sh"]`.
- Edge labels use `A -->|"unless --aws"| B`.
- Sankey is `sankey-beta` CSV: `source,target,value`. Mind map indent is two spaces per level.

## Call-chain highlight

Mermaid does not highlight a connected chain on hover. After a flowchart renders, walk its edges and fade everything outside the hovered node's callers and callees.

Behavior:

- Hover lights the node, every ancestor, every descendant, and the edges whose both ends are in that set.
- Moving the pointer off the diagram clears the highlight.
- Click pins that chain. The pinned node is filled so it is distinct from the rest of the chain. Click it again to clear. Click another node to move the pin.
- While a node is pinned, hover does not change the chain.

Parse edges from the diagram source before `mermaid.run()`. The match must not require the end of the line, because a label can follow the target id:

```js
/^\s*(\w+)\s+-->\s*(?:\|[^|]*\|\s*)?(\w+)/
```

Node ids in the SVG look like `mermaid-<n>-flowchart-<id>-<n>`. Edge paths look like `mermaid-<n>-L_<from>_<to>_<n>`. Those ids only round-trip when flowchart ids contain no underscores. Keep using camelCase ids.

Copy this CSS, scoped to the diagram container (shown as `#call-graph`):

```css
#call-graph svg .node { cursor: pointer; }
#call-graph svg.chain .node,
#call-graph svg.chain path.flowchart-link,
#call-graph svg.chain .edgeLabel {
  opacity: 0.14;
  transition: opacity 120ms ease;
}
#call-graph svg.chain .node.hot,
#call-graph svg.chain path.flowchart-link.hot,
#call-graph svg.chain .edgeLabel.hot { opacity: 1; }
#call-graph svg.chain path.flowchart-link.hot { stroke: #9a3412; }
#call-graph svg .node.pinned > rect {
  fill: #9a3412 !important;
  stroke: #7c2d12 !important;
  stroke-width: 3px;
}
#call-graph svg .node.pinned .nodeLabel { color: #fffdf8 !important; }
@media (prefers-reduced-motion: reduce) {
  #call-graph svg.chain .node,
  #call-graph svg.chain path.flowchart-link,
  #call-graph svg.chain .edgeLabel { transition: none; }
}
```

Copy this script after `mermaid.run()`. Pass the flowchart SVG and the original `<pre>` text:

```js
function highlightCallChain(svg, source) {
  const edges = [];
  for (const line of source.split('\n')) {
    const match = line.match(/^\s*(\w+)\s+-->\s*(?:\|[^|]*\|\s*)?(\w+)/);
    if (match) edges.push([match[1], match[2]]);
  }
  const nodeEls = new Map();
  for (const node of svg.querySelectorAll('g.node')) {
    const match = node.id.match(/-flowchart-(.+)-\d+$/);
    if (match) nodeEls.set(match[1], node);
  }
  const outgoing = new Map();
  const incoming = new Map();
  for (const id of nodeEls.keys()) {
    outgoing.set(id, []);
    incoming.set(id, []);
  }
  const edgeEls = edges.map(([from, to]) => {
    outgoing.get(from)?.push(to);
    incoming.get(to)?.push(from);
    const marker = `L_${from}_${to}_`;
    return {
      from,
      to,
      path: [...svg.querySelectorAll('path.flowchart-link')].find((path) => path.id.includes(marker)),
      label: svg.querySelector(`.edgeLabel .label[data-id^="${marker}"]`)?.closest('.edgeLabel')
    };
  });

  function walk(start, neighbors) {
    const seen = new Set();
    const stack = [...(neighbors.get(start) || [])];
    while (stack.length) {
      const id = stack.pop();
      if (seen.has(id)) continue;
      seen.add(id);
      stack.push(...(neighbors.get(id) || []));
    }
    return seen;
  }

  function clear() {
    svg.classList.remove('chain');
    for (const el of svg.querySelectorAll('.hot, .pinned')) {
      el.classList.remove('hot');
      el.classList.remove('pinned');
    }
  }

  let pinned = null;

  function nodeIdFrom(event) {
    const node = event.target.closest('g.node');
    if (!node) return null;
    const match = node.id.match(/-flowchart-(.+)-\d+$/);
    return match ? match[1] : null;
  }

  function show(id) {
    if (!nodeEls.has(id)) return;
    clear();
    const hot = walk(id, incoming);
    for (const next of walk(id, outgoing)) hot.add(next);
    hot.add(id);
    svg.classList.add('chain');
    for (const nodeId of hot) nodeEls.get(nodeId)?.classList.add('hot');
    if (pinned) nodeEls.get(pinned)?.classList.add('pinned');
    for (const edge of edgeEls) {
      if (!hot.has(edge.from) || !hot.has(edge.to)) continue;
      edge.path?.classList.add('hot');
      edge.label?.classList.add('hot');
    }
  }

  svg.addEventListener('mouseover', (event) => {
    if (pinned) return;
    const id = nodeIdFrom(event);
    if (id) show(id);
  });
  svg.addEventListener('mouseleave', () => {
    if (!pinned) clear();
  });
  svg.addEventListener('click', (event) => {
    const id = nodeIdFrom(event);
    if (!id) return;
    pinned = pinned === id ? null : id;
    if (pinned) show(pinned);
    else show(id);
  });
}
```

Arrowheads use a shared SVG marker, so fading a path does not fade its arrowhead. The rust stroke on hot edges is the visible emphasis.

Verify hover and click in a browser over `http://`, not `file://`. Check one node with both callers and callees, then click to pin, move the pointer away, and click again to clear.
