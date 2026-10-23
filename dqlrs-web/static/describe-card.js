const INTELLIGENT_NOTE = /^No exact match for "[^"]*", so showing (?:similar names and related keys|similar names|related keys)\.$/;

/**
 * Split an ls message into an optional intelligent-match note and either a
 * table description or a summary list.
 */
export function parseLsMessage(text) {
  const raw = String(text || "").replace(/\r\n/g, "\n").trim();
  if (!raw) return null;
  const lines = raw.split("\n");
  let note = "";
  let cache = "";
  let start = 0;
  // `ls` prints `cache: …` ahead of the table, before or after the
  // intelligent-match note. That hint is not part of the description.
  while (start < lines.length) {
    const line = lines[start].trim();
    if (!line) {
      start += 1;
      continue;
    }
    const hint = line.match(/^cache:\s*(.*)$/i);
    if (hint) {
      const text = hint[1].trim();
      if (text) cache = cache ? `${cache}\n${text}` : text;
      start += 1;
      continue;
    }
    if (!note && INTELLIGENT_NOTE.test(line)) {
      note = line;
      start += 1;
      continue;
    }
    break;
  }
  const body = lines.slice(start).join("\n").trim();
  const description = parseTableDescription(body);
  const tables = parseTableSummary(body);
  if (description) return { note, cache, description, summary: "", tables: [] };
  if (note || tables || body.startsWith("Tables")) {
    return { note, cache, description: null, summary: body, tables: tables || [] };
  }
  return null;
}

/**
 * Parse an `ls` summary table. Returns null when the text is not that table.
 * Size may contain a space (`0 B`).
 */
export function parseTableSummary(text) {
  const lines = String(text || "")
    .replace(/\r\n/g, "\n")
    .split("\n")
    .map((line) => line.trimEnd())
    .filter((line) => line.trim());
  if (lines.length < 3 || lines[0].trim() !== "Tables") return null;
  if (!/^\s*Name\s+Items\s+Read\s+Write\s+Status\s+Size\s*$/.test(lines[1])) return null;
  const rows = [];
  for (const line of lines.slice(2)) {
    const match = line.match(/^(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+(?:\s+\S+)*)$/);
    if (!match) return null;
    rows.push({
      name: match[1],
      items: match[2],
      read: match[3],
      write: match[4],
      status: match[5],
      size: match[6],
    });
  }
  return rows.length ? rows : null;
}

/**
 * Split an `ls` / `DESCRIBE` detail message into fields and the CREATE query.
 * Returns null when the text is not that description.
 */
export function parseTableDescription(text) {
  const raw = String(text || "").replace(/\r\n/g, "\n").trim();
  const lines = raw.split("\n");
  const name = lines[0] && lines[0].match(/^Name: (.+)$/);
  if (!name) return null;

  const known = new Set(["Status", "Items", "Size", "Read", "Write", "Hash Key", "Range Key"]);
  const fields = { Name: name[1] };
  const extra = [];
  const schema = [];
  let inSchema = false;
  for (const line of lines.slice(1)) {
    if (inSchema) {
      schema.push(line);
      continue;
    }
    if (line.startsWith("CREATE TABLE ")) {
      inSchema = true;
      schema.push(line);
      continue;
    }
    const labeled = line.match(/^([A-Za-z ]+): ?(.*)$/);
    if (labeled && known.has(labeled[1])) {
      fields[labeled[1]] = labeled[2];
      continue;
    }
    extra.push(line);
  }
  if (!fields["Hash Key"]) return null;

  return {
    name: fields.Name,
    status: fields.Status || "",
    items: fields.Items || "",
    size: fields.Size || "",
    read: fields.Read || "",
    write: fields.Write || "",
    hashKey: parseKey(fields["Hash Key"]),
    rangeKey: fields["Range Key"] ? parseKey(fields["Range Key"]) : null,
    ...splitIndexSections(extra.join("\n")),
    schema: schema.join("\n").trim(),
  };
}

const INDEX_STATUS = "ACTIVE|CREATING|UPDATING|DELETING";
const INDEX_RATE = "N/A|\\d+/∞|\\d+/\\d+ \\(\\d+%\\)|\\d+";
const INDEX_KEY = "\\S+ \\([^)]+\\)|-";
const GLOBAL_INDEX = new RegExp(
  `^(\\S+)\\s+(\\S+)\\s+(${INDEX_RATE})\\s+(${INDEX_RATE})\\s+(${INDEX_KEY})\\s+(${INDEX_KEY})\\s+(${INDEX_STATUS})$`,
);
const LOCAL_INDEX = /^(\S+)\s+hash=(\S+)\s+range=(\S+)\s+projection=(.*)$/;

/**
 * Pull local and global index blocks out of the describe text that follows
 * the key lines. Anything that is not an index stays in `extra`.
 */
function splitIndexSections(text) {
  const localIndexes = [];
  const globalIndexes = [];
  const rest = [];
  let section = "";
  for (const line of String(text || "").replace(/\r\n/g, "\n").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    if (trimmed === "Local Indexes:") {
      section = "local";
      continue;
    }
    if (trimmed === "Global Indexes:") {
      section = "global";
      continue;
    }
    if (section === "global" && /^Name\s+Projection\b/.test(trimmed)) continue;
    if (section === "local") {
      const row = parseLocalIndex(trimmed);
      if (row) {
        localIndexes.push(row);
        continue;
      }
    }
    if (section === "global") {
      const row = parseGlobalIndex(trimmed);
      if (row) {
        globalIndexes.push(row);
        continue;
      }
    }
    section = "";
    rest.push(line);
  }
  return {
    localIndexes,
    globalIndexes,
    extra: rest.join("\n").trim(),
  };
}

function parseLocalIndex(line) {
  const match = line.match(LOCAL_INDEX);
  if (!match) return null;
  return {
    name: match[1],
    hash: match[2],
    range: match[3] === "-" ? "" : match[3],
    projection: match[4].trim(),
  };
}

function parseGlobalIndex(line) {
  const match = line.match(GLOBAL_INDEX);
  if (!match) return null;
  return {
    name: match[1],
    projection: match[2],
    read: match[3],
    write: match[4],
    hashKey: parseKey(match[5]),
    rangeKey: match[6] === "-" ? null : parseKey(match[6]),
    status: match[7],
  };
}

function parseKey(value) {
  const match = String(value).match(/^(.*?)\s+\(([^)]+)\)$/);
  if (!match) return { name: value, type: "" };
  return { name: match[1], type: match[2] };
}
