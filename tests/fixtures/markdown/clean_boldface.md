# Configuration Reference

- **Timeout**: maximum seconds to wait before giving up on a connection attempt.
- **Retries**: number of attempts before the client reports a final failure.

The client library ships with sane defaults for most projects. Teams rarely need to touch more than a couple of settings before their first deploy. One setting worth a second look is the retry budget. A **shared** default is sometimes too slow for busy networks. Every other option can stay untouched for the life of the project.
