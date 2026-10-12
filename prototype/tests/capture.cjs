const fs = require("node:fs");
const path = require("node:path");
const { pathToFileURL } = require("node:url");
const { chromium } = require("playwright-core");

const prototypeUrl = pathToFileURL(
  path.resolve(__dirname, "..", "index.html"),
).href;
const screenshots = path.resolve(__dirname, "..", "screenshots");

(async () => {
  fs.mkdirSync(screenshots, { recursive: true });
  const browser = await chromium.launch(
    process.env.CHROME_PATH
      ? { executablePath: process.env.CHROME_PATH, headless: true }
      : { channel: "chrome", headless: true },
  );
  const page = await browser.newPage({
    viewport: { width: 1440, height: 900 },
    deviceScaleFactor: 1,
  });
  const shot = async (name) => {
    await page.screenshot({ path: path.join(screenshots, `${name}.png`) });
    console.log(`Captured ${name}.png`);
  };
  try {
    await page.goto(prototypeUrl);
    await shot("portfolio-light");
    await page.locator('.project-card[data-project="northstar"]').click();
    await shot("project-overview");
    await page.locator("#assistant-toggle").click();
    await page.locator("#panel-input").fill("Add subscriptions");
    await page.locator('#panel-form button[type="submit"]').click();
    await shot("assistant-proposal");
    await page.locator('.project-link[data-project="fieldnotes"]').click();
    await shot("fieldnotes-overview");
    await page.locator("#panel-input").fill("What should I do next?");
    await page.locator('#panel-form button[type="submit"]').click();
    await page.locator("#global-input").fill("Explain the current state");
    await page.locator('#global-form button[type="submit"]').click();
    await shot("fieldnotes-conversation");
    await page.locator("#panel-input").fill("Add subscriptions");
    await page.locator('#panel-form button[type="submit"]').click();
    await shot("fieldnotes-proposal");
    await page.locator('.project-link[data-project="northstar"]').click();
    await shot("project-switch-assistant");
    await page.getByRole("button", { name: "Work", exact: true }).click();
    await shot("work-review");
    await page.goto(
      `${prototypeUrl}?view=overview&project=northstar&mode=working&theme=dark&assistant=open`,
    );
    await shot("active-run-dark");
    await page.getByRole("button", { name: "Pause after task" }).click();
    await shot("pause-requested");
    await page.getByRole("button", { name: "Finish current task" }).click();
    await shot("paused-after-task");
    await page.goto(
      `${prototypeUrl}?view=overview&project=northstar&mode=blocked`,
    );
    await shot("blocked-decision");
    await page.goto(
      `${prototypeUrl}?view=overview&project=fieldnotes&mode=offline`,
    );
    await shot("provider-offline");
    await page.setViewportSize({ width: 700, height: 900 });
    await page.locator("#assistant-toggle").click();
    await shot("narrow-assistant");
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
