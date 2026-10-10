# P13 live smoke approval — 10 October 2026

The owner responded **“Authorised”** to the request for one disposable OpenCode test prompt to complete P13's model-driven tool and permission verification.

Scope of this single test:

- Target: a temporary Git repository created solely for the test; no production project or credential change.
- Provider and model: OpenRouter `cohere/north-mini-code:free`, explicitly selected for the prompt. OpenCode's installed model metadata and [OpenRouter's model page](https://openrouter.ai/cohere/north-mini-code%3Afree/pricing) list $0 input and output token pricing and tool-call support.
- Prompt: request one local `git status --short` tool call. The disposable project configuration makes `bash` ask for permission and denies other tools.
- Approval behavior: observe the request, exercise the application's policy mapping, reject the tool request, and stop the session. No external publication, spending, production change, or additional prompt is authorized.

The test stops after the one prompt and records its run log under `docs/evidence/`. If the free model is unavailable, no paid fallback is authorized.
