# P19 release candidate checklist

Status: **in progress** on 11 October 2026. This checklist records evidence and outstanding gates; it does not approve distribution or production use.

| Gate | Status | Evidence / next step |
| --- | --- | --- |
| Windows build and app launch | Passed portable archive smoke | [Clean-commit package log](evidence/p19-windows-package-commit.log) and [extracted app launch](evidence/p19-windows-package-smoke.md); archive SHA-256 and embedded revision verified. |
| Windows onboarding/import, planning, review, export, manual release | Partial | Earlier phase journeys passed; repeat from packaged binary before final acceptance. |
| Windows terminal and agent run/recovery | Partial | P07/P12/P14/P15 Windows evidence exists. Hands-on terminal input through UI remains pending. |
| Migration and restore | Passed local focused check | [P19 migration/restore log](evidence/p19-migration-restore.log). |
| Large-output performance | Passed local focused check | [50,000-line parser log](evidence/p19-large-output.log); full desktop responsiveness remains a manual check. |
| Secret redaction and input controls | Passed local tests; advisory review pending | [Redaction log](evidence/p19-export-redaction.log) and [focused security review](evidence/p19-security-review.md) cover credential-shaped inputs, file tampering, exact approvals, and expiry. Dependency advisory tooling is unavailable locally. |
| Accessibility | Partial | [Review](evidence/p19-accessibility-review.md): a read-only terminal transcript fallback is present; high-DPI and assistive-technology checks are pending. |
| Windows, Apple Silicon macOS, Linux CI | Pending final run | The project has a three-OS workflow; attach current commit/run link after successful execution. |
| Native M1 Max install and full journey | Unavailable | Owner's M1 Max is not yet available; must be completed before release acceptance. |
| Signing and notarization | Not configured | No signing/notarization identities have been provided. The Windows archive is unsigned and intended for local evaluation only; macOS notarization is unverified. Public distribution remains gated. |
| Scoped owner release approval | Not granted | Required before any deployment or publication. |

The final combined Windows workspace [tests](evidence/p19-final-windows-tests.log), [Clippy](evidence/p19-final-windows-clippy.log), and [format check](evidence/p19-final-windows-fmt.log) passed after the focused security repairs. The portable archive was rebuilt from clean commit `3f30b83` with an embedded provenance manifest.

No deployment, publication, purchase, or production credential change is authorized by this checklist.
