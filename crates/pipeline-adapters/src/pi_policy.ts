// Loaded explicitly by Project Pipeline. No project or user extensions load.
export default function (pi: any) {
  let modelCalls = 0;
  pi.on("before_provider_request", async (_event: any, ctx: any) => {
    if (modelCalls >= 30) {
      ctx.ui.notify("Project Pipeline model-call limit reached", "error");
      await new Promise(() => {}); // Fail closed before another provider request.
    }
    modelCalls++;
  });
  pi.on("session_start", async (_event: any, ctx: any) => {
    ctx.ui.notify("Project Pipeline policy gate ready", "info");
  });
  pi.on("tool_call", async (event: any, ctx: any) => {
    const request = JSON.stringify({ tool: event.toolName, input: event.input });
    const allowed = await ctx.ui.confirm("Project Pipeline action", request);
    if (!allowed) return { block: true, reason: "Project Pipeline policy denied this action" };
    return undefined;
  });
  pi.registerCommand("pipeline-policy-probe", {
    description: "Check the Project Pipeline RPC policy channel without model inference",
    handler: async (_args: any, ctx: any) => {
      await ctx.ui.confirm("Project Pipeline action", JSON.stringify({ tool: "read", input: { path: "." } }));
    },
  });
}
