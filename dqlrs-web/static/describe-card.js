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
    extra: extra.join("\n").trim(),
    schema: schema.join("\n").trim(),
  };
}

function parseKey(value) {
  const match = String(value).match(/^(.*?)\s+\(([^)]+)\)$/);
  if (!match) return { name: value, type: "" };
  return { name: match[1], type: match[2] };
}
