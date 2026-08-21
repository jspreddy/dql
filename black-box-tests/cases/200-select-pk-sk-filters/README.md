SELECT with partition key, sort key, and extra attribute filters.

Setup loads 1000 JSON-line rows (10 attributes: pk, sk, plus eight more)
generated once by `generate_records.py`. The query is a table Query: hash
equality, `begins_with` on the range key, then `status` and `region` as
FilterExpression so extra predicates drop some key-condition matches.
