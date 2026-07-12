# Queue Service

This service reads jobs from a queue and processes them one at a time.

It ships with robust error handling: failed jobs are retried with backoff and moved to a dead-letter queue after five attempts. The comprehensive test suite covers the retry logic, the dead-letter path, and the metrics exporter, and it runs on every pull request before merge.

Configuration lives in a small file next to the binary. See the comments in that file for the available options.
