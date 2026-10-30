# Plan: agents and workspace readers

**Group:** agents. **Status:** plan.

The query-result cache stays in the workspace. An agent that can read the project can open `.dqlrs-web-cache`. This plan says what that agent is allowed to learn, and what still requires the user.

## What encryption changes

After the encryption phase, those files are ciphertext. An agent that can read the workspace and cannot read the key store learns that a query ran, from the file name and the timestamp, and learns nothing about the statement text or the rows.

The table-definition cache is outside the workspace, under the user cache directory, and is ciphertext as well. An agent limited to the project directory does not see it at all.

## What stays possible

| Reader | Result |
| --- | --- |
| Other OS user who does not have the token | Cache directories are `0700`. Loopback API calls without the token fail. The token is not a file in the workspace |
| Agent with the project only | Sees ciphertext in `.dqlrs-web-cache`. Cannot decrypt |
| Agent with the project and the key store, or with the server's memory | Can decrypt, or can call the API. Treated as the user |
| Agent allowed to call localhost, and able to read the token from stderr or from the HTML response | Can run queries. The token is not a wall between the user and their own tools |
| MCP client the user configured | Calls `/mcp` with the token, on loopback, as it does today once the header is set |

Do not add a second credential whose only purpose is to hide data from a process the user already granted the keychain or the shell.

## Editing `.dql` files

Source files stay plaintext. The existing guidance stands: edit them on disk when the workspace is in scope, and use the MCP file tools when it is not. Lint and describe need schema. They do not need result rows. Result history is for the person at the browser.

The web UI decrypts through the server. It does not embed the cache key in `index.html` or in a static file.

## Done when

- A fixture agent that can read the workspace and cannot read the key file cannot recover a known row value from `.dqlrs-web-cache`.
- The browser, holding only the API token, still shows that same history.
- The plan's table above matches the implemented behavior, including the same-user exception.
