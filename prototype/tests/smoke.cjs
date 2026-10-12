const assert = require("node:assert/strict");
const path = require("node:path");
const { pathToFileURL } = require("node:url");
const { chromium } = require("playwright-core");

const prototypeUrl = pathToFileURL(
  path.resolve(__dirname, "..", "index.html"),
).href;

async function projectSwitchAndAssistant(page) {
  await page.goto(prototypeUrl);
  assert.match(await page.locator("#main-content").innerText(), /Northstar/);
  assert.match(await page.locator("#main-content").innerText(), /Fieldnotes/);
  await page.locator('.project-card[data-project="fieldnotes"]').click();
  assert.match(await page.locator("#breadcrumb").innerText(), /Fieldnotes/);
  assert.match(await page.locator("h1").innerText(), /research notes/i);
  assert.match(await page.locator("#main-content").innerText(), /18%/);
  await page.locator("#assistant-toggle").click();
  assert.match(
    await page.locator(".context-chip").innerText(),
    /Fieldnotes.*Overview/,
  );
  await page.locator("#panel-input").fill("What should I do next?");
  await page.locator('#panel-form button[type="submit"]').click();
  await page.locator("#global-input").fill("Explain the current state");
  await page.locator('#global-form button[type="submit"]').click();
  assert.equal(await page.locator(".assistant-message.user").count(), 2);
  await page.locator("#panel-input").fill("Unsent Fieldnotes draft");
  assert.equal(
    await page.locator("#global-input").inputValue(),
    "Unsent Fieldnotes draft",
  );
  await page.locator("#assistant-close").click();
  await page.locator("#assistant-toggle").click();
  assert.equal(
    await page.locator("#panel-input").inputValue(),
    "Unsent Fieldnotes draft",
  );
  await page.getByRole("button", { name: "Roadmap", exact: true }).click();
  assert.equal(await page.locator(".assistant-message.user").count(), 2);
  assert.match(
    await page.locator(".context-chip").innerText(),
    /Fieldnotes.*Roadmap/,
  );
  await page.locator("#panel-input").fill("Add subscriptions");
  await page.locator('#panel-form button[type="submit"]').click();
  const fieldnotesProposal = await page
    .locator(".proposal-card")
    .last()
    .getAttribute("data-proposal-id");
  assert.match(
    await page.locator(".proposal-card").last().innerText(),
    /Fieldnotes/,
  );
  await page.locator('.project-link[data-project="northstar"]').click();
  assert.match(await page.locator("#breadcrumb").innerText(), /Northstar/);
  assert.match(await page.locator("h1").innerText(), /teamwork/i);
  assert.match(
    await page.locator(".context-chip").innerText(),
    /Northstar.*Overview/,
  );
  assert.equal(await page.locator(".proposal-card").count(), 0);
  assert.equal(await page.locator("#panel-input").inputValue(), "");
  await page.evaluate((proposalId) => {
    const wrapper = document.createElement("div");
    wrapper.dataset.proposalId = proposalId;
    const button = document.createElement("button");
    button.dataset.project = "fieldnotes";
    wrapper.append(button);
    document.body.append(wrapper);
    handleAction("review-proposal", button);
    wrapper.remove();
  }, fieldnotesProposal);
  assert.match(
    await page.locator("#demo-notice").innerText(),
    /another project/i,
  );
  assert.match(await page.locator("#breadcrumb").innerText(), /Northstar/);
  await page.locator("#global-input").fill("Add subscriptions");
  await page.locator('#global-form button[type="submit"]').click();
  assert.match(
    await page.locator(".proposal-card").last().innerText(),
    /Northstar/,
  );
  assert.doesNotMatch(
    await page.locator(".proposal-card").last().innerText(),
    /Fieldnotes/,
  );
  await page.locator("#global-input").fill("Northstar draft");
  await page.locator('.project-link[data-project="fieldnotes"]').click();
  assert.equal(await page.locator("#panel-input").inputValue(), "");
  assert.equal(await page.locator(".assistant-message.user").count(), 3);
  assert.match(
    await page.locator(".proposal-card").last().innerText(),
    /Fieldnotes/,
  );
  await page.locator('.project-link[data-project="northstar"]').click();
  assert.equal(
    await page.locator("#global-input").inputValue(),
    "Northstar draft",
  );
  await page.locator('button[data-view="portfolio"]').click();
  assert.match(
    await page.locator('.project-card[data-project="northstar"]').innerText(),
    /68%/,
  );
  assert.match(
    await page.locator('.project-card[data-project="fieldnotes"]').innerText(),
    /18%/,
  );
  assert.match(
    await page.locator('.project-card[data-project="northstar"]').innerText(),
    /Awaiting review/,
  );
  assert.match(
    await page.locator('.project-card[data-project="fieldnotes"]').innerText(),
    /Ready/,
  );
  assert.match(await page.locator(".context-chip").innerText(), /All projects/);
  console.log(
    "PASS project switching, distinct context, two-message history, drafts, proposal isolation, open-panel navigation, portfolio",
  );
}

async function automationStates(page) {
  await page.goto(
    `${prototypeUrl}?view=overview&project=northstar&mode=working`,
  );
  await page.getByRole("button", { name: "Pause after task" }).click();
  assert.match(
    await page.locator("#main-content").innerText(),
    /Pause requested/,
  );
  assert.doesNotMatch(
    await page.locator("#main-content").innerText(),
    /Work is blocked by a decision/,
  );
  await page.getByRole("button", { name: "Cancel pause request" }).click();
  assert.equal(await page.locator("#state-select").inputValue(), "working");
  await page.getByRole("button", { name: "Pause after task" }).click();
  await page.getByRole("button", { name: "Finish current task" }).click();
  assert.match(
    await page.locator("#main-content").innerText(),
    /Automatic continuation is paused/,
  );
  assert.match(
    await page.locator("#main-content").innerText(),
    /needs your review/i,
  );
  assert.match(await page.locator("#main-content").innerText(), /68%/);
  await page
    .getByRole("button", { name: "Review pending work" })
    .first()
    .click();
  assert.match(await page.locator("h1").innerText(), /clearer first welcome/i);
  await page.getByRole("button", { name: "Accept work" }).click();
  assert.match(
    await page.locator("#main-content").innerText(),
    /Owner accepted this example/,
  );
  await page.getByRole("button", { name: "Overview", exact: true }).click();
  assert.match(await page.locator("#main-content").innerText(), /72%/);
  assert.match(
    await page.locator("#main-content").innerText(),
    /Automatic continuation is paused/,
  );
  await page
    .getByRole("button", { name: "Resume eligible work" })
    .first()
    .click();
  assert.equal(await page.locator("#state-select").inputValue(), "ready");
  await page
    .getByRole("button", { name: "Start approved task" })
    .first()
    .click();
  assert.equal(await page.locator("#state-select").inputValue(), "working");
  await page.getByRole("button", { name: "Pause now" }).click();
  assert.equal(await page.locator("#state-select").inputValue(), "paused");
  await page.locator("#state-select").selectOption("blocked");
  assert.match(
    await page.locator("#main-content").innerText(),
    /blocked by a decision/i,
  );
  await page.locator("#state-select").selectOption("offline");
  assert.match(
    await page.locator("#main-content").innerText(),
    /provider unavailable/i,
  );
  await page.locator("#state-select").selectOption("awaiting_review");
  assert.match(
    await page.locator("#main-content").innerText(),
    /Awaiting review/,
  );
  await page.locator("#state-select").selectOption("ready");
  assert.equal(await page.locator("#state-select").inputValue(), "ready");
  await page.locator('.project-link[data-project="fieldnotes"]').click();
  assert.equal(await page.locator("#state-select").inputValue(), "ready");
  await page.locator('.project-link[data-project="northstar"]').click();
  assert.equal(await page.locator("#state-select").inputValue(), "ready");
  await page.locator('button[data-view="portfolio"]').click();
  assert.match(
    await page.locator('.project-card[data-project="northstar"]').innerText(),
    /Ready/,
  );
  assert.match(
    await page.locator('.project-card[data-project="fieldnotes"]').innerText(),
    /Ready/,
  );
  await page.goto(
    `${prototypeUrl}?view=overview&project=northstar&mode=working`,
  );
  await page.getByRole("button", { name: "Pause after task" }).click();
  await page.getByRole("button", { name: "Finish current task" }).click();
  await page
    .getByRole("button", { name: "Resume eligible work" })
    .first()
    .click();
  assert.equal(await page.locator("#state-select").inputValue(), "ready");
  await page.getByRole("button", { name: "Review pending work" }).click();
  assert.equal(
    await page.getByRole("button", { name: "Accept work" }).count(),
    1,
  );
  console.log(
    "PASS working, pause requested, paused, resume, blocked, awaiting review, offline, ready; project state isolation",
  );
}

async function keyboardNarrowThemes(page) {
  await page.goto(prototypeUrl);
  await page.keyboard.press("Control+k");
  assert.equal(
    await page
      .locator("#global-input")
      .evaluate((element) => document.activeElement === element),
    true,
  );
  await page.locator("#theme-toggle").click();
  assert.equal(await page.locator("body").getAttribute("data-theme"), "dark");
  await page.locator("#theme-toggle").click();
  assert.equal(await page.locator("body").getAttribute("data-theme"), "light");
  await page.setViewportSize({ width: 700, height: 900 });
  await page.locator("#assistant-toggle").click();
  assert.equal(await page.locator("#assistant-panel").isVisible(), true);
  const box = await page.locator("#assistant-panel").boundingBox();
  assert.ok(box.width <= 700 && box.x > 0);
  await page.keyboard.press("Escape");
  assert.equal(await page.locator("#assistant-panel").isVisible(), false);
  assert.equal(
    await page
      .locator("#assistant-toggle")
      .evaluate((element) => document.activeElement === element),
    true,
  );
  await page.locator('.project-card[data-project="fieldnotes"]').focus();
  await page.keyboard.press("Enter");
  assert.match(await page.locator("#breadcrumb").innerText(), /Fieldnotes/);
  console.log(
    "PASS keyboard focus, Escape return, Enter project card, narrow drawer, light/dark themes",
  );
}

(async () => {
  const browser = await chromium.launch(
    process.env.CHROME_PATH
      ? { executablePath: process.env.CHROME_PATH, headless: true }
      : { channel: "chrome", headless: true },
  );
  const page = await browser.newPage({
    viewport: { width: 1440, height: 900 },
  });
  const pageErrors = [];
  page.on("pageerror", (error) => pageErrors.push(error.message));
  try {
    await projectSwitchAndAssistant(page);
    await automationStates(page);
    await keyboardNarrowThemes(page);
    assert.equal(pageErrors.length, 0, pageErrors.join("\n"));
    console.log("PASS no browser page errors");
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
