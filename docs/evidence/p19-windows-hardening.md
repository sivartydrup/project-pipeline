# P19 Windows local hardening

The Windows v11→v12 migration test builds a real v11 database, inserts project data, opens it through the current store, confirms a pre-v11 backup, restores from that backup, migrates again, and checks SQLite integrity. [Migration and restore transcript](p19-migration-restore.log).

The export test inserts credential-shaped text in free text and nested JSON and confirms that the exported JSON masks it while retaining unrelated data. Export-time redaction covers workflow tables; capability and idempotency tables remain excluded. [Redaction transcript](p19-export-redaction.log).

The terminal parser processed 50,000 lines in the bounded 24×80/5,000-row configuration. On this Windows run it took 313 ms and left 252 visible bytes containing the final line. The native PTY reader uses a 64-chunk channel and the UI drains up to 32 KiB per frame; the existing P07 test covers shell process cleanup. [Large-output transcript](p19-large-output.log).

These focused checks pass. Full workspace tests, installation smoke, three-OS CI, and native Apple Silicon checks are tracked in the [release checklist](../release-checklist.md).
