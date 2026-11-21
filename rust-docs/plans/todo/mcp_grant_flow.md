# Let the user grant an agent's query

**Priority:** Medium  
**Area:** `dqlrs web --mcp`  
**Status:** Not started. This is the workflow. The three plans below own the steps.

## Goal

An agent that needs data asks by writing a query and requesting access. The
person at the web app sees that request, inspects the query, runs it, and sees
the resulting data together with the classifier responses. Only then do they
decide whether that data goes back to the agent.

The agent does not receive the data with the request. The user sees the query
before it runs, and sees the rows and the classifier responses before deciding.
A later change of mind is visible to connected agents.

## Workflow

1. **Request.** The agent writes a query and requests access. [`mcp_grant_panel.md`](mcp_grant_panel.md)
2. **Inspect.** The request pops up. The user reads the query. [`mcp_grant_panel.md`](mcp_grant_panel.md)
3. **Run.** The user runs the query and sees the data. [`mcp_grant_panel.md`](mcp_grant_panel.md)
4. **Classify.** With that data, the user sees which attribute names matched and which values matched. [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md)
5. **Decide.** The user grants or denies. Grant sends that data back to the agent. Connected agents hear the grant, and a later revoke. [`mcp_grant_panel.md`](mcp_grant_panel.md), [`mcp_grant_events.md`](mcp_grant_events.md)

## Plans

| Plan | What it owns |
| --- | --- |
| [`mcp_grant_panel.md`](mcp_grant_panel.md) | The request pops up. The user inspects the query, runs it, sees the data, and grants or denies |
| [`mcp_grant_sensitivity.md`](mcp_grant_sensitivity.md) | The classifier responses shown with that data before the decision |
| [`mcp_grant_events.md`](mcp_grant_events.md) | After the decision, connected agents hear the grant and a later revoke |
