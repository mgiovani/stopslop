# Q3 Caching Migration Report

Over the past quarter, the platform team migrated our session store from a
single-node Redis instance to a three-node Redis cluster deployment. The
change was driven by growing latency during peak traffic windows, when the
old instance regularly saturated its connection pool and forced clients to
retry.

The migration ran in three phases. First, we stood up the new cluster
alongside the existing instance and mirrored writes to both, comparing read
results to catch any serialization drift early. Second, we moved a small
percentage of read traffic to the cluster and watched error rates and tail
latency for two weeks. Third, once the numbers held steady, we cut over the
remaining traffic and decommissioned the old instance.

Latency improved noticeably. The 99th-percentile read latency dropped from
340ms to 62ms, and connection pool exhaustion errors disappeared entirely
from the on-call rotation. The cluster also gave us headroom to grow: node
count can now scale independently of the application tier, so a traffic
spike no longer requires an emergency deploy.

It's worth noting that the migration was not without friction. Our client
library assumed a single connection endpoint, and adapting it to cluster
mode required patching the retry logic to correctly follow cluster
redirects. That patch now lives in the shared client package and benefits
every service that talks to Redis, not just the session store.

We also had to rework how we test failover locally. The old setup let
engineers run a single Redis container and call it done; the cluster setup
needs at least six nodes to represent the topology honestly, so we built a
small Docker Compose profile that spins up a minimal cluster for local
development and continuous integration.

The rollout also changed how we watch the cache layer day to day. We added
a new dashboard that tracks connection counts, replication lag, and slot
migration status across all six shards, and we wired an alert that pages
the on-call engineer whenever a shard reports more than one failed
heartbeat in a row. None of this existed before the migration; on-call
used to find out about cluster trouble from customer reports instead of
from the dashboard itself.

In conclusion, the migration met its latency and reliability goals, and the
tooling built along the way should make the next infrastructure change
easier to test and roll out safely.
