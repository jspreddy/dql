AWS_ACCESS_KEY_ID=fakeid AWS_SECRET_ACCESS_KEY=fakekey aws dynamodb \
    --endpoint-url http://localhost:8000 \
    update-table \
    --table-name foobar \
    --global-secondary-index-updates file://update-table.json
