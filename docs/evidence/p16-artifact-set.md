# P16 sample design artifact set

The `design_to_implementation_journey_preserves_reviewed_versions_and_links` test creates a sample project, active implementation task `UX-1`, and layout decision `decision`. It attaches a project-local `screen.png` design file to a `screenshot` artifact named **Home screen**, with notes, MIME type, Git commit, task, and decision provenance.

| Version | File content | Notes | Review outcome |
| --- | --- | --- | --- |
| 1 | `screen one` | Desktop layout | Changes requested: increase contrast |
| 2 | `screen two` | Contrast updated | Approved: Looks good |

The test verifies that each version has a distinct SHA-256 digest, only the owner can review the latest version, stale version actions fail, and both versions remain in the exported project record. A second test records all seven supported kinds: flow, wireframe, screenshot, component, copy, asset, and accessibility. A project-scope test rejects outside files and foreign decision links.

Execution transcript: [p16-design-journey.log](p16-design-journey.log).
