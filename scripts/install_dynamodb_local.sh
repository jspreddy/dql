#!/bin/bash

# current script path
SCRIPT_PATH=$(dirname "$0")
ROOT_PATH=$SCRIPT_PATH/..
DYNAMO_LOCAL_PATH=$ROOT_PATH/.dynamo-local

BACKGROUND_PROCESS=$1
LOCAL_HOST="${DQL_LOCAL_HOST:-localhost}"
LOCAL_PORT="${DQL_LOCAL_PORT:-8000}"

already_running() {
  python3 -c '
import socket
import sys

host, port = sys.argv[1], int(sys.argv[2])
tried = []
for candidate in (host, "127.0.0.1", "localhost"):
    if candidate in tried:
        continue
    tried.append(candidate)
    try:
        with socket.create_connection((candidate, port), timeout=1):
            sys.exit(0)
    except OSError:
        continue
sys.exit(1)
' "$LOCAL_HOST" "$LOCAL_PORT"
}

if already_running; then
  echo "DynamoDB Local already running at ${LOCAL_HOST}:${LOCAL_PORT}; not starting another instance"
  exit 0
fi

mkdir -p $DYNAMO_LOCAL_PATH
mkdir -p $DYNAMO_LOCAL_PATH/data
cd $DYNAMO_LOCAL_PATH

echo "---------------------------------------"
echo "Download and install dynamodb local    "
echo "---------------------------------------"

echo $SCRIPT_PATH
echo $ROOT_PATH
echo $DYNAMO_LOCAL_PATH


# check if tar file not exists, download it
if [ ! -f "dynamodb_local_latest.tar.gz" ]; then
    # download the zip file
    curl -O https://d1ni2b6xgvw0s0.cloudfront.net/v2.x/dynamodb_local_latest.tar.gz
    # unzip the file
    tar -xzf dynamodb_local_latest.tar.gz
else
    echo "(dynamodb_local_latest.tar.gz) already exists, skipping download"
fi

echo "---------------------------------------"
echo "Starting dynamodb local                "
echo "---------------------------------------"

if [ -z "$BACKGROUND_PROCESS" ]; then
    # start the server
    # java -Djava.library.path=$DYNAMO_LOCAL_PATH/DynamoDBLocal_lib -jar DynamoDBLocal.jar -inMemory -sharedDb -dbPath ./data
    java -Djava.library.path=$DYNAMO_LOCAL_PATH/DynamoDBLocal_lib -jar DynamoDBLocal.jar -inMemory -sharedDb
else
    # start the server in the background
    java -Djava.library.path=$DYNAMO_LOCAL_PATH/DynamoDBLocal_lib -jar DynamoDBLocal.jar -inMemory -sharedDb &
fi
