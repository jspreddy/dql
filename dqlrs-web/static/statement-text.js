// Text about DQL statements and their results: result labels, whether a statement reads or
// writes, and the counts the run dialog totals. Pure functions, no DOM.

// The server keeps the comment lines above a statement in its text. Drop them before reading the verb.
export function statementCode(statement) {
  return String(statement || "").replace(/^(?:\s*--[^\n]*(?:\n|$))*/, "").trim();
}

export function isSaveStatement(statement) {
  const text = String(statement || "")
    .replace(/--[^\n]*/g, " ")
    .replace(/'(?:[^']|\\')*'/g, " '' ")
    .replace(/"(?:[^"]|\\")*"/g, ' "" ');
  return /\bsave\s+(?:''|""|[^\s=;]+)/i.test(text);
}

/** "one row" for 1, otherwise "<n> rows". */
export function amountPhrase(count, singular, plural) {
  const amount = Number.isFinite(count) ? count : 0;
  if (amount === 1) return `one ${singular}`;
  return `${amount} ${plural}`;
}

/** Capacity units, rounded to two places. */
export function formatUnits(value) {
  const number = Number(value) || 0;
  const rounded = Math.round(number * 100) / 100;
  return String(rounded);
}

export function affectedLabel(item) {
  const count = Number(item.affected ?? 0);
  const statement = statementCode(item.statement);
  const verb = (statement.match(/^(insert|delete|update|select|scan|load)\b/i) || [])[1];
  const action = verb ? verb.toLowerCase() : "";
  if (action === "insert") return `${amountPhrase(count, "row", "rows")} inserted`;
  if (action === "delete") return `${amountPhrase(count, "row", "rows")} deleted`;
  if (action === "update") return `${amountPhrase(count, "row", "rows")} updated`;
  if (action === "load") return `${amountPhrase(count, "row", "rows")} loaded`;
  if (isSaveStatement(statement)) return `${amountPhrase(count, "row", "rows")} saved`;
  if ((action === "select" || action === "scan") && /\bcount\s*\(\s*\*\s*\)/i.test(statement)) {
    return amountPhrase(count, "row", "rows");
  }
  return `${count} affected`;
}

// DROP names a single table, so a drop that removed something dropped one table.
export function droppedLabel(item) {
  if (!/^drop\b/i.test(statementCode(item.statement))) return "";
  if (!/^Dropped\b/.test(String(item.message || ""))) return "";
  return "one table dropped";
}

/** The note text for one statement result. */
export function resultLabel(item) {
  if (!item.ok) return item.error || item.message || "failed";
  if (item.kind === "affected") return affectedLabel(item);
  if (item.kind === "status" || item.kind === "schema" || item.kind === "text") {
    return droppedLabel(item) || item.message || item.kind;
  }
  if (item.kind === "items") return ((item.items || []).length) + " rows";
  return item.message || "ok";
}

const READ_VERBS = new Set(["select", "scan"]);
const WRITE_VERBS = new Set(["insert", "update", "delete", "load", "create", "drop", "alter"]);

/** Whether a statement reads, writes, or both. SAVE writes a file, so it counts as a write. */
export function statementAccess(statement) {
  const code = statementCode(statement);
  const verb = (code.match(/^[a-z]+/i) || [""])[0].toLowerCase();
  return {
    read: READ_VERBS.has(verb),
    write: WRITE_VERBS.has(verb) || isSaveStatement(code),
  };
}

export function emptyCounts() {
  return { queries: 0, reads: 0, writes: 0, success: 0, failure: 0, readUnits: 0, writeUnits: 0 };
}

/** Add one statement result to run counts. */
export function addResultCounts(counts, result) {
  const access = statementAccess(result.statement || "");
  counts.queries += 1;
  if (access.read) counts.reads += 1;
  if (access.write) counts.writes += 1;
  if (result.ok) counts.success += 1;
  else counts.failure += 1;
  counts.readUnits += Number(result.read_units) || 0;
  counts.writeUnits += Number(result.write_units) || 0;
  return counts;
}

/** "4 queries · one read · 3 writes · 4 ok", plus "· 1 failed" when something failed. */
export function fileSummaryText(counts) {
  const parts = [
    amountPhrase(counts.queries, "query", "queries"),
    amountPhrase(counts.reads, "read", "reads"),
    amountPhrase(counts.writes, "write", "writes"),
    `${counts.success} ok`,
  ];
  if (counts.failure) parts.push(`${counts.failure} failed`);
  return parts.join(" · ");
}
