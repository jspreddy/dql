SELECT with partition key, sort key, and extra attribute filters.

Setup loads the shared 1000-row fixture (`fixtures/pk-sk-records/seed.json`).
The query is a table Query: hash equality, `begins_with` on the range key,
then `status` and `region` as FilterExpression so extra predicates drop some
key-condition matches.
