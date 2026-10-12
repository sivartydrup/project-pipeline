/* All data and actions in this file are illustrative. No service calls or persistence. */
const params = new URLSearchParams(location.search);
const allowedViews = new Set([
  "portfolio",
  "overview",
  "roadmap",
  "review",
  "projects",
  "inbox",
  "activity",
  "settings",
  "design",
  "releases",
  "history",
  "new",
]);
const allowedModes = new Set(["default", "working", "blocked", "offline"]);
const state = {
  view: allowedViews.has(params.get("view")) ? params.get("view") : "portfolio",
  project: "Northstar",
  theme: params.get("theme") === "dark" ? "dark" : "light",
  assistant: params.get("assistant") === "open",
  mode: allowedModes.has(params.get("mode")) ? params.get("mode") : "default",
  prompt: params.get("proposal") === "open" ? "Add subscriptions" : "",
  proposal: params.get("proposal") === "open",
  simulatedApproval: false,
};
document.body.dataset.theme = state.theme;
const $ = (selector) => document.querySelector(selector);
const main = $("#main-content");
const panel = $("#assistant-panel");
const escapeHtml = (value) =>
  String(value).replace(
    /[&<>"']/g,
    (char) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        char
      ],
  );
const pill = (label, type = "") =>
  `<span class="status-pill ${type}"><span class="tiny-dot"></span>${label}</span>`;
const btn = (label, action, variant = "secondary-btn") =>
  `<button class="${variant}" data-action="${action}">${label}</button>`;
const projectTabs = (active) =>
  `<nav class="project-tabs" aria-label="Project sections">${[
    ["overview", "Overview"],
    ["roadmap", "Roadmap"],
    ["review", "Work"],
    ["design", "Design"],
    ["releases", "Releases"],
    ["history", "History"],
  ]
    .map(
      ([id, label]) =>
        `<button data-view="${id}" class="${active === id ? "active" : ""}" ${active === id ? 'aria-current="page"' : ""}>${label}</button>`,
    )
    .join("")}</nav>`;
const sample = `<span class="status-pill neutral">Illustrative example</span>`;

function portfolio() {
  return `<div class="main-inner"><div class="page-head"><div><div class="overline">MONDAY, 12 OCTOBER</div><h1 class="page-title">Good morning, Studio owner.</h1><p class="page-subtitle">Here’s where your products stand, and what needs you today.</p></div>${btn("＋ New project", "new", "primary-btn")}</div><div class="card focus-card"><div class="focus-icon">✧</div><div class="focus-copy"><div class="overline">YOUR FOCUS TODAY</div><h2>Review Northstar’s latest work</h2><p>The agent finished improving onboarding. All required checks passed, and the work is ready for your decision.</p></div>${btn("Review work ↗", "review", "primary-btn")}</div><div class="section-head"><h2>Your projects</h2><button data-action="projects">View all projects →</button></div><div class="project-grid"><article class="card project-card" data-action="overview" tabindex="0" role="button" aria-label="Open Northstar"><div class="project-card-head"><span class="project-swatch teal"></span><h3>Northstar</h3>${pill("Review ready")}</div><p>A calmer way for small teams to plan and share their work.</p><div class="progress-track"><div class="progress-fill" style="width:68%"></div></div><div class="project-card-foot"><span>68% verified progress</span><span>Build stage · 1 item needs you</span></div></article><article class="card project-card" data-action="overview" tabindex="0" role="button" aria-label="Open Fieldnotes"><div class="project-card-head"><span class="project-swatch violet"></span><h3>Fieldnotes</h3>${pill("Planning", "neutral")}</div><p>A simple research journal for independent makers.</p><div class="progress-track"><div class="progress-fill" style="width:18%"></div></div><div class="project-card-foot"><span>18% verified progress</span><span>Plan ready to review</span></div></article></div><div class="section-head"><h2>Needs your attention <span style="color:var(--muted);font-weight:500">(2)</span></h2><button data-action="inbox">Open inbox →</button></div><div class="card attention-row"><div class="attention-icon">✓</div><div><strong>Northstar · Review onboarding update</strong><small>Accept the result or ask for changes. 3 checks passed.</small></div>${btn("Review", "review")}</div><div class="card attention-row"><div class="attention-icon">◇</div><div><strong>Fieldnotes · Confirm the product direction</strong><small>A draft plan is waiting for your review.</small></div>${btn("Open", "overview")}</div></div>`;
}

function overview() {
  const mode = state.mode;
  const working = mode === "working";
  const blocked = mode === "blocked";
  const offline = mode === "offline";
  const status = working
    ? pill("Working")
    : blocked
      ? pill("Blocked", "warning")
      : offline
        ? pill("Offline", "error")
        : pill("Review ready");
  return `<div class="main-inner"><div class="page-head"><div><div class="overline">PROJECT / NORTHSTAR</div><h1 class="page-title">Make teamwork feel lighter.</h1><p class="page-subtitle">Northstar is a shared planning space for small teams. ${sample}</p></div>${status}</div>${projectTabs("overview")}<div class="state-controls"><label for="state-select">Preview state</label><select id="state-select" aria-label="Preview project state"><option value="default" ${mode === "default" ? "selected" : ""}>Review ready</option><option value="working" ${working ? "selected" : ""}>Agent working</option><option value="blocked" ${blocked ? "selected" : ""}>Blocked</option><option value="offline" ${offline ? "selected" : ""}>Provider offline</option></select></div>${blocked ? `<div class="card recovery"><h3>Work is blocked by a decision</h3><p>The agent needs you to choose whether guests can join without an account. Its partial changes are preserved. Other approved tasks can still proceed.</p>${btn("Review decision", "inbox", "primary-btn")} ${btn("See preserved work", "review")}</div>` : offline ? `<div class="card recovery"><h3>AI provider unavailable</h3><p>Your project records remain available. The assistant can explain saved status, but it cannot run or generate new work until a provider is configured and reachable.</p>${btn("Open settings", "settings", "primary-btn")} ${btn("Review saved work", "review")}</div>` : ""}<div class="overview-grid" style="margin-top:${blocked || offline ? "17" : "0"}px"><div class="card next-card"><div class="overline">RECOMMENDED NEXT STEP</div><h2>${working ? "Let the current task finish" : blocked ? "Resolve the guest access decision" : offline ? "Review work already saved" : "Review the onboarding update"}</h2><p>${working ? "The agent is updating the welcome flow under the approved plan. You can inspect progress or pause safely. Completion is only recorded after your review." : blocked ? "This choice changes the onboarding experience. Review the recommendation, alternatives, and impact before work resumes." : offline ? "Three completed checks and the submitted changes are stored locally. Review them while the provider connection is unavailable." : "The agent submitted a clearer first-run experience. Required checks passed, and your acceptance is needed before this task counts toward verified progress."}</p><div class="button-row">${btn(working ? "View live work" : blocked ? "Review decision" : "Review work", working ? "working" : blocked ? "inbox" : "review", "primary-btn")}${btn("See other options", "roadmap")}</div></div><div class="card aside-card"><div class="overline">VERIFIED PROGRESS</div><div class="metric-line"><strong>68%</strong><span>of approved work accepted</span></div><div class="progress-track"><div class="progress-fill" style="width:68%"></div></div><p style="margin-top:15px">8 of 12 weighted outcomes accepted. The submitted update is still awaiting your review.</p></div></div><div class="section-head"><h2>Where you are</h2><button data-action="roadmap">View roadmap →</button></div><div class="card journey"><div class="journey-step done">Discover</div><div class="journey-step done">Plan</div><div class="journey-step current">Build</div><div class="journey-step">Release</div><div class="journey-step">Operate</div></div><div class="section-head"><h2>Current activity</h2><button data-action="review">View all work →</button></div><div class="card run-card"><div class="run-top"><span class="focus-icon" style="width:34px;height:34px;font-size:16px">${working ? "◌" : "✓"}</span><strong>${working ? "Agent updating onboarding" : "Onboarding update submitted"}</strong>${working ? pill("Working") : pill("Waiting for review", "warning")}</div><p>${working ? "The agent is improving the first welcome screen. Last verified event: source edits saved. Checks have not finished." : "A clearer welcome path and helpful empty state are ready. The agent cannot accept its own work."}</p><div class="run-bottom"><span>${working ? "OpenCode · model call 7 of 30 · 4m elapsed" : "3 checks passed · 2 files changed"}</span>${btn(working ? "Pause after task" : "Review result", working ? "pause" : "review")}</div></div></div>`;
}

function review() {
  return `<div class="main-inner"><div class="page-head"><div><div class="overline">NORTHSTAR / WORK REVIEW</div><h1 class="page-title">A clearer first welcome.</h1><p class="page-subtitle">Submitted by the agent · awaiting your review · ${sample}</p></div>${pill("Review ready")}</div>${projectTabs("review")}<div class="review-layout"><div class="card review-main"><div class="overline">WHAT CHANGED</div><h2>New members can get started with confidence.</h2><p>The first screen now explains the workspace in plain language and offers a clear next step when there are no projects yet. Two interface files changed; no external service was touched.</p><div class="section-head"><h2>Outcome checks</h2></div><ul class="check-list"><li><span class="check-mark">✓</span><div>Welcome screen explains the next action<small>Verified with screenshot and UI review.</small></div></li><li><span class="check-mark">✓</span><div>Existing onboarding path still works<small>Automated journey passed.</small></div></li><li><span class="check-mark">✓</span><div>Keyboard path reaches the primary action<small>Manual keyboard check recorded.</small></div></li></ul><details><summary>See technical details and diff</summary><p>Illustrative diff: onboarding screen copy and empty-state layout. In production, exact revision, file diff, run IDs, logs and hashes appear here.</p></details><div class="review-actions">${btn(state.simulatedApproval ? "Example marked accepted" : "Accept work", "accept", "primary-btn")}${btn("Request changes", "request")}</div><p style="margin-top:12px;font-size:11px">Accept calls the exact task-revision domain service after evidence checks. This prototype only changes its display.</p></div><aside class="card review-side"><h3>Evidence at a glance</h3><div class="evidence-item"><strong>Automated checks</strong><small>3 passed · 0 failed</small></div><div class="evidence-item"><strong>Change preview</strong><small>2 interface files · 18 additions · 6 removals</small></div><div class="evidence-item"><strong>Agent submission</strong><small>Complete; agent acceptance is not allowed</small></div><div class="evidence-item"><strong>Known risk</strong><small>Text at 200% scale needs native validation</small></div><div class="callout">After acceptance, the coordinator checks dependencies, approvals, provider limits and policy again before starting another eligible task.</div></aside></div></div>`;
}

function roadmap() {
  return `<div class="main-inner"><div class="page-head"><div><div class="overline">NORTHSTAR / ROADMAP</div><h1 class="page-title">A clear path forward.</h1><p class="page-subtitle">Only approved scope contributes to verified progress. ${sample}</p></div>${btn("Suggest a change", "proposal", "primary-btn")}</div>${projectTabs("roadmap")}<div class="card roadmap-row"><span class="roadmap-index">01</span><div><strong>Find the product direction</strong><small>Audience, problem and brief approved</small></div>${pill("Complete")}</div><div class="card roadmap-row"><span class="roadmap-index">02</span><div><strong>Make the core experience work</strong><small>8 of 12 weighted outcomes accepted · onboarding awaits review</small></div>${pill("In progress")}</div><div class="card roadmap-row"><span class="roadmap-index">03</span><div><strong>Prepare a reliable release</strong><small>Blocked until core outcomes and checks are accepted</small></div>${pill("Upcoming", "neutral")}</div><div class="section-head"><h2>Next approved work</h2></div><div class="card simple-panel"><h2>Invite teammates</h2><p>This task is ready when onboarding review is resolved. A proposed new subscription flow would be a separate scope revision and requires owner approval.</p>${btn("Ask about dependencies", "ask-next")}</div></div>`;
}

function generic(view) {
  const content = {
    projects: [
      "All projects",
      "Choose a project to see its goal, progress and next action.",
    ],
    inbox: [
      "Needs your attention",
      "Two illustrative items need an owner decision.",
    ],
    activity: [
      "Recent activity",
      "See what changed, what was verified, and what remains open.",
    ],
    settings: [
      "Autonomy and settings",
      "Guided Auto prepares recommendations and may continue approved eligible work within enforceable limits. External effects still need scoped approval.",
    ],
    design: [
      "Design",
      "Flows, visual review and accessibility evidence remain linked to approved work.",
    ],
    releases: [
      "Releases",
      "Release readiness and publication remain separate owner decisions.",
    ],
    history: [
      "History",
      "Every approval, run and outcome keeps its original record.",
    ],
    new: [
      "Start with an idea",
      "Describe what you want to make. The assistant would ask for essential context and prepare a brief for review.",
    ],
  }[view] || ["Workspace", "Choose an action to continue."];
  return `<div class="main-inner"><div class="page-head"><div><div class="overline">${view === "new" ? "NEW PROJECT" : "PROJECT PIPELINE"}</div><h1 class="page-title">${content[0]}</h1><p class="page-subtitle">${content[1]} ${sample}</p></div></div>${["design", "releases", "history"].includes(view) ? projectTabs(view) : ""}<div class="card simple-panel"><div class="empty-illustration">✧</div><h2>${view === "new" ? "What are you hoping to create?" : content[0]}</h2><p>${view === "new" ? "Use the prompt bar below. In the product, a draft brief would be previewed before it is saved or approved." : "This conceptual screen demonstrates the navigation position; detailed interaction belongs to the later UX backlog."}</p>${view === "new" ? btn("Describe an idea", "focus-composer", "primary-btn") : btn("Back to overview", "overview")}</div></div>`;
}

function assistant() {
  const context =
    state.view === "portfolio"
      ? "All projects"
      : `Northstar · ${state.view === "review" ? "Onboarding review" : state.view === "roadmap" ? "Roadmap" : "Overview"}`;
  const prompt = state.prompt;
  return `<span class="context-chip">◈ Context: ${context}</span><div class="assistant-message"><div class="message-label">ASSISTANT · ILLUSTRATIVE</div><div class="bubble">I can explain what needs attention, compare options, and prepare a change for your review. I use saved project records as context; I cannot authorize work from chat text.</div></div>${prompt ? `<div class="assistant-message user"><div class="message-label">YOU</div><div class="bubble">${escapeHtml(prompt)}</div></div><div class="assistant-message"><div class="message-label">ASSISTANT · SAMPLE RESPONSE</div><div class="bubble">${state.proposal ? "I can prepare this as a proposed roadmap change. Here is the exact preview; approved scope stays as it is until you review and approve a new revision." : "The next useful step is to review the onboarding update. The agent submitted it with three passing checks, but it still needs your acceptance. I can also show why the next task is waiting."}</div></div>` : ""}${state.proposal ? `<div class="proposal-card"><div class="overline">PROPOSED CHANGE · DRAFT ONLY</div><h3>Add subscription billing to Northstar</h3><p>Target: task plan · expected revision 4 · medium risk. Assumption: monthly team plan. No existing approved work is changed by this preview.</p><div class="proposal-diff"><div><strong>Before</strong>Core team planning, no billing work</div><div><strong>Proposed</strong>New billing discovery and implementation milestone</div></div><ul><li>Requires your review of the revised brief and plan.</li><li>New scope is not scheduled until exact-revision approval.</li><li>Can be discarded without changing the current plan.</li></ul>${btn("Refine proposal", "focus-panel")}${btn("Review proposed scope", "proposal-review", "primary-btn")}</div>` : ""}<div class="suggested"><button data-action="ask-next">What should I do next?</button><button data-action="proposal">Add subscriptions</button><button data-action="offline">What if the provider is offline?</button></div>`;
}

function render() {
  main.innerHTML =
    state.view === "portfolio"
      ? portfolio()
      : state.view === "overview"
        ? overview()
        : state.view === "review"
          ? review()
          : state.view === "roadmap"
            ? roadmap()
            : generic(state.view);
  $("#breadcrumb").innerHTML =
    state.view === "portfolio"
      ? "Workspace <span>/</span> Home"
      : `Workspace <span>/</span> ${["projects", "inbox", "activity", "settings", "new"].includes(state.view) ? state.view[0].toUpperCase() + state.view.slice(1) : `Northstar <span>/</span> ${state.view[0].toUpperCase() + state.view.slice(1)}`}`;
  document
    .querySelectorAll(".nav-item[data-view]")
    .forEach((el) =>
      el.classList.toggle("active", el.dataset.view === state.view),
    );
  document.querySelector(".project-link small").textContent =
    state.mode === "working"
      ? "Working"
      : state.mode === "blocked"
        ? "Blocked"
        : state.mode === "offline"
          ? "Offline"
          : "Review";
  $("#theme-toggle").textContent = state.theme === "light" ? "☾" : "☀";
  $("#theme-toggle").setAttribute(
    "aria-label",
    `Switch to ${state.theme === "light" ? "dark" : "light"} theme`,
  );
  panel.hidden = !state.assistant;
  $("#assistant-toggle").setAttribute("aria-expanded", String(state.assistant));
  $("#assistant-content").innerHTML = assistant();
}
function navigate(view) {
  state.view = view;
  if (view === "overview" || view === "portfolio") state.mode = "default";
  render();
  main.scrollTop = 0;
  main.focus({ preventScroll: true });
}
function openAssistant(focus = true) {
  state.assistant = true;
  render();
  if (focus) $("#panel-input").focus();
}
function handleAction(action) {
  if (
    [
      "portfolio",
      "projects",
      "inbox",
      "activity",
      "settings",
      "overview",
      "roadmap",
      "review",
      "design",
      "releases",
      "history",
      "new",
    ].includes(action)
  ) {
    navigate(action);
    return;
  }
  if (action === "proposal") {
    state.prompt = "Add subscriptions";
    state.proposal = true;
    openAssistant(false);
    return;
  }
  if (action === "ask-next") {
    state.prompt = "What should I do next?";
    state.proposal = false;
    openAssistant(false);
    return;
  }
  if (action === "offline") {
    state.mode = "offline";
    navigate("overview");
    state.mode = "offline";
    render();
    return;
  }
  if (action === "working") {
    state.mode = "working";
    navigate("overview");
    state.mode = "working";
    render();
    return;
  }
  if (action === "proposal-review") {
    navigate("roadmap");
    openAssistant(false);
    return;
  }
  if (action === "accept") {
    state.simulatedApproval = true;
    render();
    return;
  }
  if (action === "request") {
    state.prompt = "Request changes: make the welcome text clearer";
    openAssistant();
    return;
  }
  if (action === "pause") {
    state.mode = "blocked";
    render();
    return;
  }
  if (action === "focus-composer") {
    $("#global-input").focus();
    return;
  }
  if (action === "focus-panel") {
    $("#panel-input").focus();
    return;
  }
}
document.addEventListener("click", (event) => {
  const target = event.target.closest("[data-action],[data-view]");
  if (target) handleAction(target.dataset.action || target.dataset.view);
});
document.addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    $("#global-input").focus();
  }
  if (event.key === "Escape" && state.assistant) {
    state.assistant = false;
    render();
    $("#assistant-toggle").focus();
  }
  if (
    (event.key === "Enter" || event.key === " ") &&
    event.target.matches(".project-card")
  ) {
    event.preventDefault();
    handleAction(event.target.dataset.action);
  }
});
$("#assistant-toggle").addEventListener("click", () => {
  state.assistant = !state.assistant;
  render();
  if (state.assistant) $("#panel-input").focus();
});
$("#assistant-close").addEventListener("click", () => {
  state.assistant = false;
  render();
  $("#assistant-toggle").focus();
});
$("#theme-toggle").addEventListener("click", () => {
  state.theme = state.theme === "light" ? "dark" : "light";
  document.body.dataset.theme = state.theme;
  render();
});
$("#global-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const text = $("#global-input").value.trim();
  if (!text) return;
  state.prompt = text;
  state.proposal = /add|subscription|change|revise|create/i.test(text);
  $("#global-input").value = "";
  openAssistant(false);
});
$("#panel-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const text = $("#panel-input").value.trim();
  if (!text) return;
  state.prompt = text;
  state.proposal = /add|subscription|change|revise|create/i.test(text);
  $("#panel-input").value = "";
  render();
  $("#panel-input").focus();
});
document.addEventListener("change", (event) => {
  if (event.target.id === "state-select") {
    state.mode = event.target.value;
    render();
  }
});
render();
