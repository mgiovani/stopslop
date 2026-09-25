# Monitoring Pipeline

The monitoring pipeline collects request latency samples every second,
forwards them through a small in-process buffer, batches them into one
minute windows, writes each window to the metrics store, and then a
background job scans the stored windows for the previous six hours to
compute rolling percentiles that feed the alerting rules the on-call
team configured last spring.

<!-- expect-line: 3 SLOP033 -->

The on-call runbook nests each remediation step under the alert that triggers it.

- Detect the incident from the paging alert first
  - Open the runbook for the affected service
    - Check the dashboards for anomalies before paging anyone else
      - Restart the affected worker pool if the dashboards confirm the outage
        - The rollback job reads the last healthy release tag from the registry, drains traffic from every canary node in the affected region, redeploys the previous build to each node one at a time, waits for the health probe to pass twice in a row, and then restores traffic while posting a summary of each step to the incident channel for the team

<!-- expect-line: 18 SLOP033 -->
