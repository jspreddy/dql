UPDATE every loaded row except the two from `200-select-pk-sk-filters`, adding
a new `patched` attribute. Check with `SCAN count(*)` filtered by
`attribute_exists(patched)` (998 of 1000).

Uses the shared fixture `fixtures/pk-sk-records/seed.json`.
