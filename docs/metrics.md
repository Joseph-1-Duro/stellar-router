# Transaction Success Metrics

Track and query success/failure rates for routed calls using router-middleware.

## How It Works
router-middleware already emits pre_call and post_call events on every routed
call. An off-chain indexer (e.g. a Node.js service subscribed to Stellar RPC
event streams) can consume these events and store metrics in a database.

## Event Schema
pre_call  → { caller: Address, route: String }
post_call → { caller: Address, route: String, success: bool }

## Suggested Metrics to Track
- success_rate per route (succeeded / total)
- failure_count per route per window
- top callers by volume
- circuit breaker trip frequency

## Integration Example (Node.js)
const server = new StellarRpc.Server("https://soroban-testnet.stellar.org");
const events = await server.getEvents({ filters: [{ type: "contract", contractIds: [MIDDLEWARE_ID] }] });
for (const event of events.events) {
  const topic = event.topic[0].value;
  if (topic === "post_call") {
    const [caller, route, success] = event.value.value;
    db.record({ caller, route, success, timestamp: event.ledger });
  }
}

## Querying Metrics
SELECT route, COUNT(*) as total,
       SUM(CASE WHEN success THEN 1 ELSE 0 END) as succeeded,
       ROUND(100.0 * SUM(CASE WHEN success THEN 1 ELSE 0 END) / COUNT(*), 2) as success_rate
FROM call_events
GROUP BY route
ORDER BY total DESC;
