# Notes on the New Caching Layer

This document walks through the caching layer added to the ingestion service last quarter and why the team chose the current design over the earlier prototype.

The original prototype used a simple hash map with no eviction policy which worked fine in testing but quickly ran into trouble once real traffic arrived. The rewrite delves into the request patterns collected over three months of production logs to figure out which keys actually needed to stay warm. That analysis unveiled a genuinely intricate access pattern: a small meticulous set of hot keys accounted for most of the read volume while a long tail of cold keys barely mattered.

The new implementation showcases a two-tier structure. A small in-memory ring buffer holds the hottest entries and a disk-backed layer handles everything else. The interplay between the two tiers is what makes the design multifaceted rather than a single blunt cache.

The migration also elucidates a myriad of smaller decisions that would otherwise be easy to miss. Deciding how to size the ring buffer, when to promote a cold key to hot status, and how much headroom to leave for a traffic spike all took real thought. Each of those choices was nuanced enough that the team wrote a short design note explaining the reasoning since the paradigm behind the eviction policy is not obvious from the code alone.

The synergy between the ring buffer and the disk layer boasts a measurable win. Median read latency dropped by roughly forty percent under replayed production traffic once the change landed. Load on the origin database also eased once the cache started absorbing the burgeoning read volume from the mobile client which had been growing steadily for months.

A veritable pile of edge cases turned up during testing most of them around cache invalidation during a deploy. The aforementioned ring buffer handles most of these cleanly by simply dropping entries on restart rather than trying to keep them consistent across a version change. Overall the rewrite gives the ingestion service a more capable answer to a problem that had been growing for a long time and the team plans to reuse the same two-tier structure in the next service that needs a cache.

<!-- expect-line: 5 SLOP016 -->
