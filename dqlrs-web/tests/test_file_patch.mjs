import assert from "node:assert/strict";
import test from "node:test";

import { unifiedDiff } from "../static/file-patch.js";

test("unifiedDiff is a patch of the unsaved lines", () => {
  const patch = unifiedDiff(
    "-- editable\nSELECT 1;\n",
    "-- editable\nSELECT 1;\n-- local unsaved\n",
    "queries/edit.dql",
  );
  assert.equal(
    patch,
    [
      "--- a/queries/edit.dql",
      "+++ b/queries/edit.dql",
      "@@ -1,2 +1,3 @@",
      " -- editable",
      " SELECT 1;",
      "+-- local unsaved",
      "",
    ].join("\n"),
  );
});

test("unifiedDiff is empty when the text matches", () => {
  assert.equal(unifiedDiff("SELECT 1;\n", "SELECT 1;\n", "a.dql"), "");
});

test("unifiedDiff notes a trailing newline on its own", () => {
  const patch = unifiedDiff("SELECT 1;", "SELECT 1;\n", "a.dql");
  assert.match(patch, /^--- a\/a\.dql\n\+\+\+ b\/a\.dql\n/);
  assert.match(patch, /Newline added at end of file/);
});
