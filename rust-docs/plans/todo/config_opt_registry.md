# TODO: Config and `opt` registry

**Priority:** Medium  
**Smell / SOLID:** Duplicated schemas, weak typing, OCP for options  
**Crates:** `dql-cli`, `dql-output`

## Problem

Option and flag metadata are defined in multiple places and can drift:

- `lib.rs` `KNOWN_FLAGS` pre-scan vs `clap` `CliArgs`
- `CliConfig::public_keys` / `get_value`
- `meta/opt.rs` `print_option` / `set_option`
- `width`, `pagesize`, `_throttle` stored as `serde_json::Value` instead of
  typed fields
- `OutputConfig` maps from those JSON values via `WidthSetting::from_config`

`ReplHandler` forces a uniform `(session, args, kwargs, out, repl)` signature
even when parameters are unused; REPL vs one-shot policy is scattered inside
handlers.

## Recommendation

1. Introduce an `OptRegistry` (name, getter, setter, choices, help) used by
   `opt`, config introspection, and help text.
2. Type `width` / `pagesize` as enums (`Auto | Fixed(u16)`); keep throttle in
   `TableLimits` as the single runtime source of truth (persist only at
   save/load boundaries).
3. Derive or generate unknown-flag handling from `CliArgs` instead of a manual
   `KNOWN_FLAGS` list, or document why the pre-scan must stay.
4. Pass an `ExecutionContext` into meta handlers (or split traits) so unused
   `repl`/`out` flags are not threaded everywhere.

## Acceptance

- Adding a new `opt` key requires one registry entry, not four match arms.
- Config round-trip tests still pass; `opt` help lists match registry names.
