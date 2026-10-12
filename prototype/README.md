# Modern Product Studio prototype

Open `index.html` in a current browser, or serve this directory with `python -m http.server 8000` and open `http://localhost:8000/prototype/` from the repository root.

This is an **isolated, illustrative** interaction prototype. Its projects, agent activity, checks, and approvals are sample data; no provider, database, scheduler, or project files are connected. Buttons that represent consequential actions show the intended review state but cannot approve or execute real work.

Try the Portfolio, Overview, Work review, and Roadmap navigation; expand/collapse the assistant; enter “add subscriptions” or use a suggested prompt; switch Light/Dark; use the state selector for Working, Blocked, and Offline. `Ctrl/Cmd+K` focuses the persistent composer; `Escape` closes the assistant and returns focus to its toggle. At a narrow width the assistant becomes an overlay drawer.

Screenshots are in `screenshots/`. Open at 1440×900 for the closest comparison. Review direct states with `?view=overview`, `?view=review`, `?view=overview&assistant=open&proposal=open`, `?view=overview&mode=working&theme=dark`, `?view=overview&mode=blocked`, and `?view=overview&mode=offline`.
