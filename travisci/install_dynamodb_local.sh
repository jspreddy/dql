#!/bin/bash

# current script path
SCRIPT_PATH=$(dirname "$0")
ROOT_PATH=$SCRIPT_PATH/..
DYNAMO_LOCAL_PATH=$ROOT_PATH/.dynamo-local



mkdir -p $DYNAMO_LOCAL_PATH
mkdir -p $DYNAMO_LOCAL_PATH/data
cd $DYNAMO_LOCAL_PATH

echo "---------------------------------------"
echo "Download and install dynamodb local    "
echo "---------------------------------------"

echo $SCRIPT_PATH
echo $ROOT_PATH
echo $DYNAMO_LOCAL_PATH



# download the zip file
curl -O https://d1ni2b6xgvw0s0.cloudfront.net/v2.x/dynamodb_local_latest.tar.gz

# unzip the file
tar -xzf dynamodb_local_latest.tar.gz

echo "---------------------------------------"
echo "Starting dynamodb local                "
echo "---------------------------------------"  

# start the server
java -Djava.library.path=$DYNAMO_LOCAL_PATH/DynamoDBLocal_lib -jar DynamoDBLocal.jar -sharedDb -dbPath ./data
