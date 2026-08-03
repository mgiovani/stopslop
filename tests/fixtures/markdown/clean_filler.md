# Q3 Load Testing Report

The load testing pipeline runs nightly against a staging replica of the production database, replaying anonymized traffic captured from the previous week to catch regressions before they reach customers.

The simply typed lambda calculus is unrelated to this report, but the team named the internal test harness after it as an inside joke, and the name stuck through several rewrites of the tooling.

Engineers on the team are not just testing raw throughput; they are also testing failure recovery, restart behavior, and the retry logic that kicks in when a downstream dependency times out mid-request.

When it comes to interpreting the results, the on-call engineer cross-references three separate dashboards before filing a ticket, which keeps false alarms low even during noisy weeks.
