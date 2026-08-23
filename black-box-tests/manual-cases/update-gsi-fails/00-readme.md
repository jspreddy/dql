# Create Table - Success

```sql
CREATE TABLE foobar (id STRING HASH KEY, foo NUMBER) GLOBAL INDEX ('foo_index', foo, THROUGHPUT(1, 1));
```

## Update GSI - Fails
```sql
ALTER TABLE foobar SET INDEX foo_index THROUGHPUT (2, 2);
```

It should not fail, but it does. I tried this with aws cli and it fails there as well with the same error.

```
An error occurred (InternalFailure) when calling the UpdateTable operation (reached max retries: 2): The request processing has failed because of an unknown error, exception or failure.
```

The issue seems to be from dynamodb-local bug. Here is someone else with the same issue and no resolution yet.
https://dba.stackexchange.com/questions/340820/unable-to-create-a-gsi-on-dynamodb-from-aws-cli-on-local


try the aws cli route:

```bash
./exec_test.sh
```
