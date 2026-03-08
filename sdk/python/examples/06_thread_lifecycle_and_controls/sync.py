from codex_app_server import Codex


with Codex() as codex:
    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
    _ = codex.thread_list(limit=20)
    _ = thread.read(include_turns=False)
    print("Lifecycle OK:", thread.id)
