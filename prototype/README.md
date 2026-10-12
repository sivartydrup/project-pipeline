# Modern Product Studio prototype

Open `index.html` in a current browser, or serve this directory with `python -m http.server 8000` and open `http://localhost:8000/prototype/` from the repository root.

This is an **isolated, illustrative** interaction prototype. Its projects, agent activity, checks, and approvals are sample data; no provider, database, scheduler, or project files are connected. Buttons that represent consequential actions show the intended review state but cannot approve or execute real work.

Try both project cards: Northstar opens a Build-stage review; Fieldnotes opens a Planning-stage research project with its own progress and next action. The global prompt and expanded panel share a draft for the current context. Send several messages, leave an unsent draft, switch projects, and return; each project keeps its own conversation and draft during the open browser session. "Add subscriptions" creates a project-bound illustrative proposal.

Use the Overview state selector to preview Ready, Working, Pause requested, Paused, Blocked, Awaiting review and Offline. In Working, choose **Pause after task** and then **Finish current task** to see the task finish while automatic continuation remains paused; review remains separate. **Pause now** safely stops the simulated run, and **Resume eligible work** returns to Ready after a stated fresh preflight. `Ctrl/Cmd+K` focuses the persistent composer; `Escape` closes the assistant and returns focus to its toggle. At a narrow width the assistant becomes an overlay drawer. Switch Light/Dark with the toolbar button.

Screenshots are in `screenshots/`. Open at 1440×900 for the closest comparison. Direct states use `?view=overview&project=fieldnotes`, `?view=overview&project=northstar&mode=working`, and other `mode=` values listed above. Prototype state persists while the page stays open; reloading resets the illustrative scenario.

To run browser verification or regenerate screenshots: from `prototype/`, run `npm ci`, then `npm test` or `npm run screenshots`. The tests use an installed Chrome browser through `playwright-core`. Set `CHROME_PATH` if Chrome is not on Playwright's default path.
