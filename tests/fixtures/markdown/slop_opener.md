# Notes on Caching

Here's the thing: caches expire when nobody expects it.

This is the part most people skip when tuning eviction policies.

Think about it: most outages start from a stale value.

Why does this matter? Because latency drops immediately.

<!-- expect-line: 3 SLOP022 -->
<!-- expect-line: 5 SLOP022 -->
<!-- expect-line: 7 SLOP022 -->
<!-- expect-line: 9 SLOP022 -->
