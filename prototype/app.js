/* Isolated interaction model: every record and action here is illustrative. */
const projectData = {
  northstar: {
    id: "northstar",
    name: "Northstar",
    swatch: "teal",
    stage: "Build",
    goal: "Make teamwork feel lighter.",
    description: "A shared planning space for small teams.",
    portfolioDescription:
      "A calmer way for small teams to plan and share their work.",
    progress: 68,
    accepted: "8 of 12 weighted outcomes accepted",
    task: "Improve the first welcome",
    reviewTitle: "A clearer first welcome.",
    reviewOutcome: "New members can get started with confidence.",
    reviewSummary:
      "The first screen explains the workspace in plain language and offers a clear next step when there are no projects yet. Two interface files changed; no external service was touched.",
    checks: [
      "Welcome screen explains the next action",
      "Existing onboarding path still works",
      "Keyboard path reaches the primary action",
    ],
    risk: "Text at 200% scale needs native validation",
    decision: "Choose whether guests can join without an account",
    approvedWork: "Invite teammates",
    before: "Core team planning, no billing work",
    planRevision: 4,
    roadmap: [
      "Find the product direction",
      "Make the core experience work",
      "Prepare a reliable release",
    ],
  },
  fieldnotes: {
    id: "fieldnotes",
    name: "Fieldnotes",
    swatch: "violet",
    stage: "Planning",
    goal: "Turn research notes into useful insight.",
    description: "A simple research journal for independent makers.",
    portfolioDescription: "A simple research journal for independent makers.",
    progress: 18,
    accepted: "2 of 11 weighted outcomes accepted",
    task: "Synthesize interview notes",
    reviewTitle: "Research themes are ready.",
    reviewOutcome: "The first audience patterns are easier to compare.",
    reviewSummary:
      "The agent grouped illustrative interview notes into themes and linked each finding to its source. No project files or external services were changed.",
    checks: [
      "Themes link to source notes",
      "Uncertain findings are labeled",
      "Summary remains editable",
    ],
    risk: "Two interview notes still need owner confirmation",
    decision: "Confirm which audience segment comes first",
    approvedWork: "Synthesize interview notes",
    before: "Research journal plan, no billing work",
    planRevision: 2,
    roadmap: [
      "Understand the audience",
      "Shape the journal experience",
      "Validate the first release",
    ],
  },
};

const projectViews = new Set([
  "overview",
  "roadmap",
  "review",
  "design",
  "releases",
  "history",
]);
const allViews = new Set([
  "portfolio",
  "projects",
  "inbox",
  "activity",
  "settings",
  "new",
  ...projectViews,
]);
const runStates = [
  "ready",
  "working",
  "pause_requested",
  "paused",
  "blocked",
  "awaiting_review",
  "offline",
];
const params = new URLSearchParams(location.search);
const initialProject = projectData[params.get("project")]
  ? params.get("project")
  : "northstar";
const state = {
  view: allViews.has(params.get("view")) ? params.get("view") : "portfolio",
  selectedProject: initialProject,
  theme: params.get("theme") === "dark" ? "dark" : "light",
  assistantOpen: params.get("assistant") === "open",
  drafts: { portfolio: "", northstar: "", fieldnotes: "" },
  conversations: { portfolio: [], northstar: [], fieldnotes: [] },
  proposals: {},
  nextProposal: 1,
  projects: {
    northstar: {
      runState: "awaiting_review",
      reviewPending: true,
      reviewAccepted: false,
    },
    fieldnotes: {
      runState: "ready",
      reviewPending: false,
      reviewAccepted: false,
    },
  },
  notice: "",
};
if (runStates.includes(params.get("mode"))) {
  state.projects[initialProject].runState = params.get("mode");
  state.projects[initialProject].reviewPending =
    params.get("mode") === "awaiting_review";
}
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
const currentProject = () => projectData[state.selectedProject];
const currentRun = () => state.projects[state.selectedProject];
const contextKey = () =>
  projectViews.has(state.view) ? state.selectedProject : "portfolio";
const contextLabel = () =>
  contextKey() === "portfolio"
    ? "All projects"
    : `${currentProject().name} · ${state.view === "review" ? "Work review" : state.view[0].toUpperCase() + state.view.slice(1)}`;
const statusLabels = {
  ready: "Ready",
  working: "Working",
  pause_requested: "Pause requested",
  paused: "Paused",
  blocked: "Blocked",
  awaiting_review: "Awaiting review",
  offline: "Offline",
};
const statusTypes = {
  pause_requested: "warning",
  paused: "neutral",
  blocked: "warning",
  awaiting_review: "warning",
  offline: "error",
};
const pill = (label, type = "") =>
  `<span class="status-pill ${type}"><span class="tiny-dot"></span>${label}</span>`;
const statePill = (run) =>
  pill(statusLabels[run.runState], statusTypes[run.runState] || "");
const btn = (label, action, variant = "secondary-btn", projectId = "") =>
  `<button class="${variant}" data-action="${action}" ${projectId ? `data-project="${projectId}"` : ""}>${label}</button>`;
const sample = pill("Illustrative example", "neutral");
const projectTabs = (active) =>
  `<nav class="project-tabs" aria-label="${currentProject().name} sections">${[
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
const projectProgress = (id) =>
  projectData[id].progress + (state.projects[id].reviewAccepted ? 4 : 0);
const activeTask = (project, run) =>
  run.reviewAccepted && run.runState === "ready"
    ? project.approvedWork
    : project.task;
const acceptedSummary = (project) =>
  state.projects[project.id].reviewAccepted
    ? project.id === "northstar"
      ? "9 of 12 weighted outcomes accepted"
      : "3 of 11 weighted outcomes accepted"
    : project.accepted;

function recommendation(project, run) {
  if (run.runState === "working")
    return {
      title: `Follow current work on ${project.name}`,
      reason: `The approved task, ${project.task.toLowerCase()}, is running within its current limits. It still needs review after submission.`,
      action: "View current work",
      target: "overview",
    };
  if (run.runState === "pause_requested")
    return {
      title: "Let this task finish, then pause",
      reason:
        "The current approved task continues. No new task will start after it finishes; pausing does not accept or reject the result.",
      action: "View current work",
      target: "overview",
    };
  if (run.runState === "paused")
    return {
      title: run.reviewPending
        ? "Review saved work while automation is paused"
        : "Resume eligible work when ready",
      reason: run.reviewPending
        ? "The finished task awaits your decision. Automatic continuation remains off."
        : "No task is running. Resume will recheck approvals, dependencies, permissions, provider and limits.",
      action: run.reviewPending ? "Review work" : "Resume eligible work",
      target: run.reviewPending ? "review" : "resume",
    };
  if (run.runState === "blocked")
    return {
      title: project.decision,
      reason:
        "An unresolved product decision prevents this work from continuing. Saved work is preserved.",
      action: "Review decision",
      target: "inbox",
    };
  if (run.runState === "awaiting_review")
    return {
      title:
        project.id === "northstar"
          ? "Review the onboarding update"
          : "Review the research themes",
      reason:
        "The agent submitted work with illustrative checks. Your acceptance is required before it counts as verified progress.",
      action: "Review work",
      target: "review",
    };
  if (run.runState === "offline")
    return {
      title: run.reviewPending
        ? "Review saved work while the provider is offline"
        : "Continue planning while the provider is offline",
      reason:
        "Project records remain available locally. New agent work cannot start while the configured provider is unavailable.",
      action: run.reviewPending ? "Review saved work" : "Open saved plan",
      target: run.reviewPending ? "review" : "roadmap",
    };
  if (run.reviewPending)
    return {
      title: `Review ${project.task.toLowerCase()}`,
      reason:
        "A submitted result still needs owner acceptance. Other approved, eligible work may start only after a fresh policy check.",
      action: "Review work",
      target: "review",
    };
  return {
    title: `Start approved work: ${activeTask(project, run)}`,
    reason:
      "This illustrative task is approved, ready and unblocked. Guided Auto may start eligible work only after rechecking provider/model authorization, limits, permissions and dependencies.",
    action: "Start approved task",
    target: "start",
  };
}

function projectCard(project) {
  const run = state.projects[project.id];
  const progress = projectProgress(project.id);
  return `<article class="card project-card" data-view="overview" data-project="${project.id}" tabindex="0" role="button" aria-label="Open ${project.name}"><div class="project-card-head"><span class="project-swatch ${project.swatch}"></span><h3>${project.name}</h3>${statePill(run)}</div><p>${project.portfolioDescription}</p><div class="progress-track"><div class="progress-fill" style="width:${progress}%"></div></div><div class="project-card-foot"><span>${progress}% verified progress</span><span>${project.stage} stage · ${statusLabels[run.runState]}</span></div></article>`;
}

function portfolio(showFocus = true) {
  const prioritized = ["northstar", "fieldnotes"].sort((a, b) => {
    const priority = {
      awaiting_review: 0,
      blocked: 1,
      pause_requested: 2,
      working: 3,
      offline: 4,
      paused: 5,
      ready: 6,
    };
    return (
      priority[state.projects[a].runState] -
      priority[state.projects[b].runState]
    );
  });
  const focusProject = projectData[prioritized[0]];
  const focus = recommendation(focusProject, state.projects[focusProject.id]);
  return `<div class="main-inner"><div class="page-head"><div><div class="overline">ILLUSTRATIVE STUDIO · 12 OCTOBER</div><h1 class="page-title">${showFocus ? "Good morning, Studio owner." : "Your projects"}</h1><p class="page-subtitle">${showFocus ? "Here’s where your products stand, and what needs you today." : "Choose a project to see its own goal, progress and next action."}</p></div>${btn("＋ New project", "new", "primary-btn")}</div>${showFocus ? `<div class="card focus-card"><div class="focus-icon">✧</div><div class="focus-copy"><div class="overline">YOUR FOCUS TODAY · ${focusProject.name.toUpperCase()}</div><h2>${focus.title}</h2><p>${focus.reason}</p></div>${btn(`${focus.action} ↗`, focus.target, "primary-btn", focusProject.id)}</div>` : ""}<div class="section-head"><h2>Your projects</h2><button data-action="projects">View all projects →</button></div><div class="project-grid">${Object.values(projectData).map(projectCard).join("")}</div>${showFocus ? `<div class="section-head"><h2>Needs your attention</h2><button data-action="inbox">Open inbox →</button></div><div class="card attention-row"><div class="attention-icon">✓</div><div><strong>Northstar · ${recommendation(projectData.northstar, state.projects.northstar).title}</strong><small>${statusLabels[state.projects.northstar.runState]} · ${projectData.northstar.stage} stage</small></div>${btn("Open", "overview", "secondary-btn", "northstar")}</div><div class="card attention-row"><div class="attention-icon">◇</div><div><strong>Fieldnotes · ${recommendation(projectData.fieldnotes, state.projects.fieldnotes).title}</strong><small>${statusLabels[state.projects.fieldnotes.runState]} · ${projectData.fieldnotes.stage} stage</small></div>${btn("Open", "overview", "secondary-btn", "fieldnotes")}</div>` : ""}</div>`;
}

function runActions(project, run) {
  if (run.runState === "working")
    return `${btn("Pause after task", "pause-after")}${btn("Pause now", "pause-now")}${btn("Finish current task", "finish-task", "text-btn")}`;
  if (run.runState === "pause_requested")
    return `${btn("Cancel pause request", "cancel-pause")}${btn("Pause now", "pause-now")}${btn("Finish current task", "finish-task", "text-btn")}`;
  if (run.runState === "paused")
    return `${btn("Resume eligible work", "resume", "primary-btn")}${run.reviewPending ? btn("Review pending work", "review") : ""}`;
  if (run.runState === "ready")
    return `${btn("Start approved task", "start", "primary-btn")}${run.reviewPending ? btn("Review pending work", "review") : ""}`;
  if (run.runState === "awaiting_review")
    return btn("Review result", "review", "primary-btn");
  if (run.runState === "blocked")
    return btn("Review decision", "inbox", "primary-btn");
  return btn("Open settings", "settings", "primary-btn");
}

function overview() {
  const project = currentProject();
  const run = currentRun();
  const rec = recommendation(project, run);
  const progress = projectProgress(project.id);
  const notice =
    run.runState === "blocked"
      ? `<div class="card recovery"><h3>Work is blocked by a decision</h3><p>${project.decision}. The affected work is preserved; other approved tasks can proceed if policy permits.</p>${btn("Review decision", "inbox", "primary-btn")}</div>`
      : run.runState === "offline"
        ? `<div class="card recovery"><h3>Configured provider unavailable</h3><p>Saved project records are still available. Deterministic guidance and manual review continue; no new agent run is implied.</p>${btn("Open settings", "settings", "primary-btn")}</div>`
        : run.runState === "paused"
          ? `<div class="card recovery"><h3>Automatic continuation is paused</h3><p>No new task will start. ${run.reviewPending ? "A submitted result still needs your review." : "Partial work and evidence remain saved."} Resume performs a fresh eligibility and safety check.</p>${runActions(project, run)}</div>`
          : "";
  return `<div class="main-inner"><div class="page-head"><div><div class="overline">PROJECT / ${project.name.toUpperCase()}</div><h1 class="page-title">${project.goal}</h1><p class="page-subtitle">${project.description} ${sample}</p></div>${statePill(run)}</div>${projectTabs("overview")}<div class="state-controls"><label for="state-select">Preview state</label><select id="state-select" aria-label="Preview ${project.name} automation state">${runStates.map((value) => `<option value="${value}" ${run.runState === value ? "selected" : ""}>${statusLabels[value]}</option>`).join("")}</select></div>${notice}<div class="overview-grid" style="margin-top:${notice ? "17" : "0"}px"><div class="card next-card"><div class="overline">RECOMMENDED NEXT STEP</div><h2>${rec.title}</h2><p>${rec.reason}</p><div class="button-row">${btn(rec.action, rec.target, "primary-btn")}${btn("See other options", "roadmap")}</div></div><div class="card aside-card"><div class="overline">VERIFIED PROGRESS</div><div class="metric-line"><strong>${progress}%</strong><span>of approved work accepted</span></div><div class="progress-track"><div class="progress-fill" style="width:${progress}%"></div></div><p style="margin-top:15px">${acceptedSummary(project)}. ${run.reviewPending ? "Submitted work still awaits owner acceptance." : "Only accepted outcomes count."}</p></div></div><div class="section-head"><h2>Where you are</h2><button data-action="roadmap">View roadmap →</button></div><div class="card journey">${["Discover", "Plan", "Build", "Release", "Operate"].map((stage, index) => `<div class="journey-step ${index < (project.stage === "Build" ? 2 : 1) ? "done" : stage === (project.stage === "Planning" ? "Plan" : project.stage) ? "current" : ""}">${stage}</div>`).join("")}</div><div class="section-head"><h2>Current activity</h2><button data-action="review">View work →</button></div><div class="card run-card"><div class="run-top"><span class="focus-icon" style="width:34px;height:34px;font-size:16px">${["working", "pause_requested"].includes(run.runState) ? "◌" : "✓"}</span><strong>${activeTask(project, run)}</strong>${statePill(run)}</div><p>${run.runState === "working" ? "The approved task is actively running. Last verified event: source edits saved; checks have not finished." : run.runState === "pause_requested" ? "The current task continues. No new task will start when it ends; the result will still need review." : run.runState === "paused" ? "Automatic continuation is off. Saved work remains available for review or a safe resume." : run.runState === "awaiting_review" ? "The agent submitted its result. It cannot accept its own work." : run.runState === "ready" ? "Approved work is eligible. Starting still requires a fresh preflight." : run.runState === "blocked" ? "An unresolved decision prevents this task from progressing." : "The provider is unavailable; no task is running."}</p><div class="run-bottom"><span>${["working", "pause_requested"].includes(run.runState) ? "OpenCode · model call 7 of 30 · 4m elapsed" : run.reviewPending ? "3 illustrative checks passed · owner review pending" : "Project state saved locally"}</span><div class="run-buttons">${runActions(project, run)}</div></div></div></div>`;
}

function review() {
  const project = currentProject();
  const run = currentRun();
  if (!run.reviewPending && !run.reviewAccepted)
    return `<div class="main-inner"><div class="page-head"><div><div class="overline">${project.name.toUpperCase()} / WORK</div><h1 class="page-title">No work awaiting review.</h1><p class="page-subtitle">${project.task} is ${statusLabels[run.runState].toLowerCase()}. ${sample}</p></div></div>${projectTabs("review")}<div class="card simple-panel"><h2>Review begins after submission</h2><p>An agent result must include outcome checks and evidence. It cannot accept itself. You can return to the project overview for its current state.</p>${btn("Back to overview", "overview", "primary-btn")}</div></div>`;
  return `<div class="main-inner"><div class="page-head"><div><div class="overline">${project.name.toUpperCase()} / WORK REVIEW</div><h1 class="page-title">${project.reviewTitle}</h1><p class="page-subtitle">${run.reviewAccepted ? "Illustratively accepted by owner" : "Submitted by the agent · awaiting your review"} · ${sample}</p></div>${pill(run.reviewAccepted ? "Accepted" : "Awaiting review", run.reviewAccepted ? "" : "warning")}</div>${projectTabs("review")}<div class="review-layout"><div class="card review-main"><div class="overline">WHAT CHANGED</div><h2>${project.reviewOutcome}</h2><p>${project.reviewSummary}</p><div class="section-head"><h2>Outcome checks</h2></div><ul class="check-list">${project.checks.map((check) => `<li><span class="check-mark">✓</span><div>${check}<small>Illustrative evidence available for owner inspection.</small></div></li>`).join("")}</ul><details><summary>See technical details and diff</summary><p>Illustrative diff and run details for ${project.name}. In production, exact revision, file diff, run IDs, logs and hashes appear here.</p></details><div class="review-actions">${run.reviewAccepted ? pill("Owner accepted this example") : `${btn("Accept work", "accept", "primary-btn")}${btn("Request changes", "request")}`}</div><p style="margin-top:12px;font-size:11px">The real product would call the exact task-revision domain service after evidence checks. This prototype changes display state only.</p></div><aside class="card review-side"><h3>Evidence at a glance</h3><div class="evidence-item"><strong>Outcome checks</strong><small>${project.checks.length} illustrative checks passed</small></div><div class="evidence-item"><strong>Change preview</strong><small>${project.id === "northstar" ? "2 interface files" : "Research notes and theme summary"}</small></div><div class="evidence-item"><strong>Agent submission</strong><small>Complete; agent acceptance is not allowed</small></div><div class="evidence-item"><strong>Known risk</strong><small>${project.risk}</small></div><div class="callout">After acceptance, the coordinator rechecks approvals, dependencies, permissions, provider and limits before any eligible work continues.</div></aside></div></div>`;
}

function roadmap() {
  const project = currentProject();
  const run = currentRun();
  return `<div class="main-inner"><div class="page-head"><div><div class="overline">${project.name.toUpperCase()} / ROADMAP</div><h1 class="page-title">A clear path forward.</h1><p class="page-subtitle">Only accepted work in approved scope contributes to progress. ${sample}</p></div>${btn("Suggest a change", "proposal", "primary-btn")}</div>${projectTabs("roadmap")}${project.roadmap.map((title, index) => `<div class="card roadmap-row"><span class="roadmap-index">0${index + 1}</span><div><strong>${title}</strong><small>${index === 0 ? "Approved direction and evidence saved" : index === 1 ? `${acceptedSummary(project)} · ${statusLabels[run.runState]}` : "Upcoming after prerequisites are accepted"}</small></div>${pill(index === 0 ? "Complete" : index === 1 ? "In progress" : "Upcoming", index === 2 ? "neutral" : "")}</div>`).join("")}<div class="section-head"><h2>Next approved work</h2></div><div class="card simple-panel"><h2>${project.approvedWork}</h2><p>${recommendation(project, run).reason} A proposed subscription flow would be a separate scope revision requiring owner approval.</p>${btn("Ask about this work", "ask-next")}</div></div>`;
}

function generic(view) {
  if (view === "projects") return portfolio(false);
  if (view === "inbox")
    return `<div class="main-inner"><div class="page-head"><div><div class="overline">ALL PROJECTS</div><h1 class="page-title">Needs your attention</h1><p class="page-subtitle">Illustrative project-specific requests and work states.</p></div></div>${Object.values(
      projectData,
    )
      .map(
        (project) =>
          `<div class="card attention-row"><div class="attention-icon">◇</div><div><strong>${project.name} · ${recommendation(project, state.projects[project.id]).title}</strong><small>${project.stage} · ${statusLabels[state.projects[project.id].runState]}</small></div>${btn("Open project", "overview", "secondary-btn", project.id)}</div>`,
      )
      .join("")}</div>`;
  const project = currentProject();
  const global = !projectViews.has(view);
  const titles = {
    activity: [
      "Recent activity",
      "Illustrative activity across both projects remains separated by project.",
    ],
    settings: [
      "Autonomy and settings",
      "Guided Auto may continue approved eligible work after a fresh safety check. External effects still need scoped approval.",
    ],
    new: [
      "Start with an idea",
      "Describe what you want to make; a draft brief would be previewed before approval.",
    ],
    design: [
      "Design",
      `${project.name} design evidence stays linked to its approved work.`,
    ],
    releases: [
      "Releases",
      `${project.name} release readiness and publication require separate owner decisions.`,
    ],
    history: [
      "History",
      `${project.name} approvals, runs and outcomes keep their original records.`,
    ],
  };
  const [title, subtitle] = titles[view] || [
    "Workspace",
    "Choose an action to continue.",
  ];
  return `<div class="main-inner"><div class="page-head"><div><div class="overline">${global ? "PROJECT PIPELINE" : project.name.toUpperCase()}</div><h1 class="page-title">${title}</h1><p class="page-subtitle">${subtitle} ${sample}</p></div></div>${global ? "" : projectTabs(view)}<div class="card simple-panel"><div class="empty-illustration">✧</div><h2>${view === "new" ? "What are you hoping to create?" : title}</h2><p>${view === "new" ? "Use the prompt bar below. This design phase does not save or approve a real brief." : "This conceptual screen preserves the selected context; detailed workflow belongs to later UX implementation."}</p>${btn(view === "new" ? "Describe an idea" : global ? "Back to portfolio" : "Back to overview", view === "new" ? "focus-composer" : global ? "portfolio" : "overview", "primary-btn")}</div></div>`;
}

function proposalCard(proposal) {
  return `<div class="proposal-card" data-proposal-id="${proposal.id}"><div class="overline">PROPOSED CHANGE · DRAFT ONLY</div><h3>${escapeHtml(proposal.title)}</h3><p>Project: <strong>${proposal.projectName}</strong> · prepared from ${proposal.screen} · target: task plan · expected revision ${proposal.expectedRevision} · medium risk. This preview cannot change approved scope.</p><div class="proposal-diff"><div><strong>Before</strong>${proposal.before}</div><div><strong>Proposed</strong>${proposal.after}</div></div><ul><li>Review revised direction and plan before approval.</li><li>New work cannot be scheduled before exact-revision approval.</li><li>Discarding this preview leaves current scope unchanged.</li></ul>${btn("Refine proposal", "focus-panel")}${btn("Review proposed scope", "review-proposal", "primary-btn", proposal.projectId)}</div>`;
}

function assistant() {
  const key = contextKey();
  const messages = state.conversations[key];
  const welcome =
    key === "portfolio"
      ? "I can summarize the illustrative portfolio. Choose a project before proposing a change to its plan."
      : `I can explain ${currentProject().name}’s saved state, compare options, and prepare a project-specific draft for review. Conversation text cannot authorize work.`;
  return `<span class="context-chip">◈ Context: ${escapeHtml(contextLabel())}</span><div class="assistant-message"><div class="message-label">ASSISTANT · DEMONSTRATION</div><div class="bubble">${welcome}</div></div>${messages.map((message) => `<div class="assistant-message ${message.role === "user" ? "user" : ""}"><div class="message-label">${message.role === "user" ? "YOU" : "ASSISTANT · SIMULATED"}</div><div class="bubble">${escapeHtml(message.text)}</div>${message.proposalId ? proposalCard(state.proposals[message.proposalId]) : ""}</div>`).join("")}<div class="suggested"><button data-action="ask-next">What should I do next?</button><button data-action="proposal">Add subscriptions</button><button data-action="ask-offline">What if the provider is offline?</button></div>`;
}

function render() {
  const view = state.view;
  main.innerHTML =
    view === "portfolio"
      ? portfolio()
      : view === "overview"
        ? overview()
        : view === "review"
          ? review()
          : view === "roadmap"
            ? roadmap()
            : generic(view);
  const project = currentProject();
  $("#breadcrumb").innerHTML = projectViews.has(view)
    ? `Workspace <span>/</span> ${project.name} <span>/</span> ${view === "review" ? "Work" : view[0].toUpperCase() + view.slice(1)}`
    : `Workspace <span>/</span> ${view === "portfolio" ? "Home" : view[0].toUpperCase() + view.slice(1)}`;
  document
    .querySelectorAll(".nav-item[data-view]")
    .forEach((item) =>
      item.classList.toggle(
        "active",
        (view === "portfolio" ? "portfolio" : view) === item.dataset.view,
      ),
    );
  document.querySelectorAll(".project-link[data-project]").forEach((item) => {
    const id = item.dataset.project;
    item.querySelector("small").textContent =
      statusLabels[state.projects[id].runState];
    item.classList.toggle(
      "active-project",
      projectViews.has(view) && id === state.selectedProject,
    );
  });
  $("#theme-toggle").textContent = state.theme === "light" ? "☾" : "☀";
  $("#theme-toggle").setAttribute(
    "aria-label",
    `Switch to ${state.theme === "light" ? "dark" : "light"} theme`,
  );
  $("#global-context").textContent =
    contextKey() === "portfolio" ? "All projects" : project.name;
  $("#global-input").value = state.drafts[contextKey()];
  $("#panel-input").value = state.drafts[contextKey()];
  $("#panel-input").placeholder =
    `Ask about ${contextKey() === "portfolio" ? "your projects" : project.name}…`;
  panel.hidden = !state.assistantOpen;
  $("#assistant-toggle").setAttribute(
    "aria-expanded",
    String(state.assistantOpen),
  );
  $("#assistant-content").innerHTML = assistant();
  $("#demo-notice").hidden = !state.notice;
  $("#demo-notice").textContent = state.notice;
}

function navigate(view, projectId) {
  if (projectId && projectData[projectId]) state.selectedProject = projectId;
  state.view = view;
  state.notice = "";
  render();
  main.scrollTop = 0;
  main.focus({ preventScroll: true });
}

function sendMessage(text) {
  const key = contextKey();
  const intent = /add|subscription|change|revise|create/i.test(text);
  state.conversations[key].push({ role: "user", text });
  let response;
  let proposalId;
  if (key === "portfolio") {
    response = intent
      ? "Choose Northstar or Fieldnotes before preparing a plan change. This portfolio conversation has no authority to change either project."
      : "Northstar has work awaiting owner review. Fieldnotes has an approved research task ready to start. Open either project for its own records and next action.";
  } else {
    const project = projectData[key];
    const run = state.projects[key];
    if (intent) {
      proposalId = `${key}-${state.nextProposal++}`;
      state.proposals[proposalId] = {
        id: proposalId,
        projectId: key,
        projectName: project.name,
        screen: state.view,
        title: `Add subscriptions to ${project.name}`,
        expectedRevision: project.planRevision,
        before: project.before,
        after: "New billing discovery and implementation milestone",
      };
      response = `I prepared a draft change for ${project.name}. Its approved scope stays unchanged until you review the exact project revision and approve through the existing gates.`;
    } else if (/offline|provider/i.test(text)) {
      response = `${project.name} records stay available locally when the provider is offline. No new agent work starts; saved reviews and deterministic guidance remain available.`;
    } else {
      const rec = recommendation(project, run);
      response = `${project.name}: ${rec.title}. ${rec.reason} This is an illustrative answer based on the selected project’s displayed state.`;
    }
  }
  state.conversations[key].push({
    role: "assistant",
    text: response,
    proposalId,
  });
  state.drafts[key] = "";
  state.assistantOpen = true;
  render();
  $("#assistant-content").scrollTop = $("#assistant-content").scrollHeight;
}

function setRunState(next) {
  const run = currentRun();
  run.runState = next;
  if (next === "awaiting_review") run.reviewPending = true;
  state.notice =
    "All automation transitions here are simulated; the real coordinator would recheck policy and evidence.";
  render();
}

function handleAction(action, element) {
  const targetProject = element?.dataset?.project;
  if (allViews.has(action)) {
    navigate(action, targetProject);
    return;
  }
  if (action === "proposal") {
    sendMessage("Add subscriptions");
    return;
  }
  if (action === "ask-next") {
    sendMessage("What should I do next?");
    return;
  }
  if (action === "ask-offline") {
    sendMessage("What if the provider is offline?");
    return;
  }
  if (action === "review-proposal") {
    const proposalId =
      element?.closest("[data-proposal-id]")?.dataset.proposalId;
    const proposal = state.proposals[proposalId];
    if (
      !proposal ||
      proposal.projectId !== state.selectedProject ||
      proposal.projectId !== contextKey() ||
      targetProject !== proposal.projectId
    ) {
      state.notice =
        "This proposal belongs to another project. Return to its original conversation to review it.";
      render();
      return;
    }
    navigate("roadmap", proposal.projectId);
    state.assistantOpen = true;
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
  if (action === "start" && currentRun().runState === "ready") {
    setRunState("working");
    return;
  }
  if (action === "pause-after" && currentRun().runState === "working") {
    setRunState("pause_requested");
    return;
  }
  if (
    action === "cancel-pause" &&
    currentRun().runState === "pause_requested"
  ) {
    setRunState("working");
    return;
  }
  if (
    action === "pause-now" &&
    ["working", "pause_requested"].includes(currentRun().runState)
  ) {
    setRunState("paused");
    return;
  }
  if (
    action === "finish-task" &&
    ["working", "pause_requested"].includes(currentRun().runState)
  ) {
    const wasPauseRequested = currentRun().runState === "pause_requested";
    currentRun().reviewPending = true;
    setRunState(wasPauseRequested ? "paused" : "awaiting_review");
    return;
  }
  if (action === "resume" && currentRun().runState === "paused") {
    setRunState("ready");
    return;
  }
  if (action === "accept" && currentRun().reviewPending) {
    currentRun().reviewAccepted = true;
    currentRun().reviewPending = false;
    if (currentRun().runState === "awaiting_review")
      currentRun().runState = "ready";
    state.notice =
      "Illustrative owner acceptance recorded. Any automatic continuation would recheck every approval, dependency, permission and limit.";
    render();
    return;
  }
  if (action === "request" && currentRun().reviewPending) {
    state.notice =
      "In the real product, requesting changes would record a reason and keep the result unaccepted. This prototype makes no project change.";
    render();
  }
}

document.addEventListener("click", (event) => {
  const target = event.target.closest("[data-action],[data-view]");
  if (target)
    handleAction(target.dataset.action || target.dataset.view, target);
});
document.addEventListener("input", (event) => {
  if (!["global-input", "panel-input"].includes(event.target.id)) return;
  const draft = event.target.value;
  state.drafts[contextKey()] = draft;
  (event.target.id === "global-input"
    ? $("#panel-input")
    : $("#global-input")
  ).value = draft;
});
document.addEventListener("change", (event) => {
  if (event.target.id !== "state-select") return;
  const next = event.target.value;
  if (!runStates.includes(next)) return;
  currentRun().runState = next;
  currentRun().reviewPending = next === "awaiting_review";
  state.notice = "Preview state changed for this illustrative project only.";
  render();
});
document.addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    $("#global-input").focus();
  }
  if (event.key === "Escape" && state.assistantOpen) {
    state.assistantOpen = false;
    render();
    $("#assistant-toggle").focus();
  }
  if (
    (event.key === "Enter" || event.key === " ") &&
    event.target.matches(".project-card")
  ) {
    event.preventDefault();
    handleAction("overview", event.target);
  }
});
$("#assistant-toggle").addEventListener("click", () => {
  state.assistantOpen = !state.assistantOpen;
  render();
  if (state.assistantOpen) $("#panel-input").focus();
});
$("#assistant-close").addEventListener("click", () => {
  state.assistantOpen = false;
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
  const text = state.drafts[contextKey()].trim();
  if (text) sendMessage(text);
});
$("#panel-form").addEventListener("submit", (event) => {
  event.preventDefault();
  const text = state.drafts[contextKey()].trim();
  if (text) {
    sendMessage(text);
    $("#panel-input").focus();
  }
});
if (params.get("proposal") === "open") sendMessage("Add subscriptions");
else render();
