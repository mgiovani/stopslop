# Notes on Caching

It's important to note that caching reduces database load significantly.

In conclusion, the cache should be enabled for all production services.

First and foremost, monitor hit rates before tuning eviction policies.

A cold start might potentially slow down the first few requests after deploy.

<!-- expect-line: 3 SLOP015 -->
<!-- expect-line: 9 SLOP015 -->
