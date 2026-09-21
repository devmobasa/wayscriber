# Memory optimization — V6 status

The [V6 assignment](implementation-plan-v6.md) follows the
[V5 review](review-v5.md). The current
[V6 acceptance matrix and raw evidence](../../../../docs/memory-optimization/v6-acceptance-2026-09-21.md)
supersede older open-status wording for the newly exercised cells.
The evidence repository's reviewed handoff revision is `3a07011`; application
source remains at `2cad646f`.

The held-buffer resize and fractional-scale checks passed for Built-in and GTK
toolbars in isolated runs, including two repeated cycles and late releases from
retired generations. Isolated capture-timeout recovery, mixed-output switching,
several zoom controls, and region-picker cancellation were also exercised.
Physical tablet, native dual monitor, IME composition, live tray actions,
old-overlay/new-broker coexistence,
and the V2 latency/resource budgets remain open.

No app source, installation, service, buffer count, or renderer default changed
in V6. The supported same-workload overall saving remains **12.3%**. The user
explicitly deferred the remaining cells for release scope on 2026-09-21. The
tested scope is ready for another agent's review; original full V2 acceptance
remains **incomplete**. No release was authorized or performed.
