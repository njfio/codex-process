from pathlib import Path

from codex_app_server import (
    Codex,
    TextInput,
    TurnAskForApproval,
    TurnPersonality,
    TurnReasoningEffort,
    TurnReasoningSummary,
    TurnSandboxPolicy,
)

OUTPUT_SCHEMA = {
    "type": "object",
    "properties": {
        "summary": {"type": "string"},
        "actions": {
            "type": "array",
            "items": {"type": "string"},
        },
    },
    "required": ["summary", "actions"],
    "additionalProperties": False,
}

SANDBOX_POLICY = TurnSandboxPolicy.model_validate(
    {
        "type": "readOnly",
        "access": {"type": "fullAccess"},
    }
)
SUMMARY = TurnReasoningSummary.model_validate("concise")

PROMPT = (
    "Analyze a safe rollout plan for enabling a feature flag in production. "
    "Return JSON matching the requested schema."
)

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})

    turn = thread.turn(
        TextInput(PROMPT),
        approval_policy=TurnAskForApproval.never,
        cwd=str(Path.cwd()),
        effort=TurnReasoningEffort.medium,
        model="gpt-5",
        output_schema=OUTPUT_SCHEMA,
        personality=TurnPersonality.pragmatic,
        sandbox_policy=SANDBOX_POLICY,
        summary=SUMMARY,
    )
    result = turn.run()

    print("Status:", result.status)
    print("Text:", result.text)
    print("Usage:", result.usage)
