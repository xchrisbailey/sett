# Vendor comparison

I compared the three options against the criteria we agreed on in [[Storage requirements]].

| Vendor | Price / TB | Regions | Egress fees | Notes |
| --- | ---: | :---: | --- | --- |
| Alpha Storage | $4.00 | 12 | none | Strong S3 compatibility |
| Bravo Cloud | $5.50 | 30 | $0.01/GB | Best latency in APAC |
| Charlie Blob | $3.20 | 6 | $0.02/GB | Cheapest, but *no* versioning |

**Recommendation:** Alpha Storage. It has no egress fees, and `rclone` already supports it.

| Criterion | Weight |
|-----------|--------|
| Cost | 40% |
| Compatibility | 35% |
| Reach | 25% |
