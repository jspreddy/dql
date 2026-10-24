// A unified diff of unsaved editor text. Kept in the browser only.

export function unifiedDiff(before, after, path) {
  if (before === after) return "";
  const older = splitLines(before);
  const newer = splitLines(after);
  const ops = older.lines.length * newer.lines.length > 250000
    ? replaceAll(older.lines, newer.lines)
    : lineDiff(older.lines, newer.lines);
  const hunks = formatHunks(ops);
  let note = "";
  if (older.newline !== newer.newline) {
    note = newer.newline
      ? "\\ Newline added at end of file\n"
      : "\\ Newline removed at end of file\n";
  }
  if (!hunks && !note) return "";
  return `--- a/${path}\n+++ b/${path}\n${hunks}${note}`;
}

function splitLines(text) {
  const newline = text.endsWith("\n");
  const body = newline ? text.slice(0, -1) : text;
  return { lines: body === "" ? [] : body.split("\n"), newline };
}

function replaceAll(before, after) {
  return [
    ...before.map((line) => ["-", line]),
    ...after.map((line) => ["+", line]),
  ];
}

function lineDiff(before, after) {
  const n = before.length;
  const m = after.length;
  const dp = Array.from({ length: n + 1 }, () => new Uint32Array(m + 1));
  for (let i = n - 1; i >= 0; i -= 1) {
    const row = dp[i];
    const next = dp[i + 1];
    for (let j = m - 1; j >= 0; j -= 1) {
      row[j] = before[i] === after[j] ? next[j + 1] + 1 : Math.max(next[j], row[j + 1]);
    }
  }
  const ops = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (before[i] === after[j]) {
      ops.push([" ", before[i]]);
      i += 1;
      j += 1;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push(["-", before[i]]);
      i += 1;
    } else {
      ops.push(["+", after[j]]);
      j += 1;
    }
  }
  while (i < n) ops.push(["-", before[i++]]);
  while (j < m) ops.push(["+", after[j++]]);
  return ops;
}

function formatHunks(ops) {
  const placed = [];
  let oldLine = 1;
  let newLine = 1;
  for (const [kind, text] of ops) {
    placed.push({ kind, text, oldLine, newLine });
    if (kind !== "+") oldLine += 1;
    if (kind !== "-") newLine += 1;
  }
  let out = "";
  let index = 0;
  while (index < placed.length) {
    if (placed[index].kind === " ") {
      index += 1;
      continue;
    }
    let end = index + 1;
    while (end < placed.length) {
      let look = end;
      let gap = 0;
      while (look < placed.length && placed[look].kind === " ") {
        gap += 1;
        look += 1;
      }
      if (look >= placed.length) {
        end = placed.length;
        break;
      }
      if (gap > 6) break;
      end = look + 1;
    }
    const from = Math.max(0, index - 3);
    const to = Math.min(placed.length, end + 3);
    out += formatHunk(placed.slice(from, to));
    index = to;
  }
  return out;
}

function formatHunk(lines) {
  const oldCount = lines.filter((line) => line.kind !== "+").length;
  const newCount = lines.filter((line) => line.kind !== "-").length;
  const oldStart = oldCount === 0 ? 0 : lines.find((line) => line.kind !== "+").oldLine;
  const newStart = newCount === 0 ? 0 : lines.find((line) => line.kind !== "-").newLine;
  const body = lines.map((line) => `${line.kind}${line.text}\n`).join("");
  return `@@ -${oldStart},${oldCount} +${newStart},${newCount} @@\n${body}`;
}
