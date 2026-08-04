# Monitoring Pipeline

The monitoring pipeline collects request latency samples every second,
forwards them through a small in-process buffer, batches them into one
minute windows, writes each window to the metrics store, and then a
background job scans the stored windows for the previous six hours to
compute rolling percentiles that feed the alerting rules the on-call
team configured last spring.

<!-- expect-line: 3 SLOP033 -->
