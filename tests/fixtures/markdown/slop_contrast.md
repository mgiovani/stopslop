# Incident Review

It's not a caching bug. It's a race in the invalidation path.

The problem isn't the query planner. The problem is a missing index on the join column.

Not a framework. Not a library. A compiler for the test suite.

<!-- expect-line: 3 SLOP023 -->
<!-- expect-line: 5 SLOP023 -->
<!-- expect-line: 7 SLOP023 -->
