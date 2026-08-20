# TODO: Deduplicate cross-cutting helpers

**Priority:** Medium  
**Smell / SOLID:** DRY, mixed ownership  
**Crates:** `dql-engine`, `dql-expr`, `dql-cli`, `dql-output`

## Problem

Several small utilities are copy-pasted or owned in two places:

| Concern | Locations |
| --- | --- |
| Base64 encode/decode | `convert.rs`, `json_util.rs` (hand-rolled) |
| Item → JSON | `json_util`, `file_io` compact lines |
| AWS client build | `aws.rs::build_client`, `cloudwatch.rs::build_client` |
| `parse_bool` | `meta/ls.rs`, `meta/opt.rs` (different signatures) |
| Display / buffer backend | `session`, `meta/file`, `repl::BufferBackend` |
| Selection field extract | `dql-expr`, `dql-models` (see typed AST todo) |
| Throttle + meta cache | engine + SDK (see engine boundaries todo) |

## Recommendation

1. Use the `base64` crate or one shared codec module; route file JSON lines
   through shared `json_util` helpers.
2. Share one `load_sdk_config(&SdkConfig)` for DynamoDB and CloudWatch.
3. Add `meta/args.rs` with `parse_bool_kw` / `parse_bool_required`.
4. Centralize display-backend construction once CLI pipelines are unified.
5. Do not invent a third selection-field extractor — fold into typed AST work.

## Acceptance

- One base64 implementation; one AWS config loader; one bool-parse helper.
- No behavioral change in SAVE/LOAD or CloudWatch paths under existing tests.
