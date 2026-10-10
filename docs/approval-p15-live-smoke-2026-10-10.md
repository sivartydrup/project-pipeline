# P15 live Windows sample approval — 10 October 2026

The owner answered **“Approve this P15 run”** in this conversation to the exact scoped request: one disposable Windows Pi run against the approved addition sample using OpenRouter `cohere/north-mini-code:free`, up to 15 assistant model calls, a 160,000 observed-token stop, and the existing 15-minute wall stop. The run is limited to the isolated addition checkout. Shell actions still need separate exact owner approval. There is no paid fallback.

Disposable project `d628d859-1ade-458f-98fc-395aa99bd264`, task `fix-add`, was created through `ProjectEngine` in a temporary local folder. Its included addition test fails against the committed subtraction bug. The Pi 1.1.0 model catalog lists the exact free route; [OpenRouter pricing](https://openrouter.ai/cohere/north-mini-code%3Afree/pricing) lists zero input and output price. The route must be rechecked immediately before dispatch.

This approval is for the P15 sample only. It does not transfer P14's remaining call allowance, approve shell commands, or authorize production effects, publication, credentials, or paid models.

After that run stopped at its wall limit, the owner approved one more run with the same route and limits. It fixed the sample and passed its test but reached the 15-call cap before bridge submission. On 10 October 2026 the owner then approved a 30-call fail-closed cap and one additional disposable Windows run on the same verified-free route, with the same 160,000 observed-token and 15-minute stops, no paid fallback, and separate exact owner approval for every shell action. The prior two run approvals are spent and do not authorize further runs.

The 30-call run also stopped before submission. It fixed and tested the sample and reached the bridge, exposing incorrect use of the reply's project revision in place of the task revision. That run approval is spent. A proposed run with OpenRouter `poolside/laguna-s-2.1:free` awaits separate owner approval; do not dispatch it based on the approvals above.
