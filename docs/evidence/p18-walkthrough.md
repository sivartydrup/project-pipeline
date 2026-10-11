# P18 sample idea-to-operation walkthrough

The `p18_walkthrough` example created one isolated in-memory project through the store service and exported its full workflow record. It:

1. Captured an idea and approved brief revision 1.
2. Approved a one-task plan, verified its criterion with evidence, and accepted the task.
3. Bound a release to a full commit and hashed build/check files, approved readiness and separate execution, then recorded a successful manual deployment and smoke result. The example records a manual result; it does not deploy anything.
4. Recorded positioning, launch copy, a hashed launch asset, and a channel plan. It approved exact positioning content and target before recording manual publication; it did not publish externally.
5. Recorded a signup metric, feedback, bug, resolved incident, and maintenance item in the same project activity timeline.

The [sample export](p18-sample-export.json) reports all nine lifecycle kinds, 28 activity events, one approved brief, an accepted task, deployed release record, published positioning record, and zero unresolved blockers. The export includes the underlying approval, artifact, release, lifecycle, task, and activity tables. This is sample data only.
