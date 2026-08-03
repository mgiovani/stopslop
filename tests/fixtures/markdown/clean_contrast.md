# Client Library Notes

It's a small utility that wraps the retry logic, and it's used by every client in the fleet.

That patch benefits every service that talks to the queue, not just the worker pool.

The client keeps a short in-memory cache and refreshes it on a fixed interval.
