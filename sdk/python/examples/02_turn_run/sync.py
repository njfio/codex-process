from codex_app_server import Codex, TextInput

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
    result = thread.turn(TextInput("Give 3 bullets about SIMD.")).run()

    print("thread_id:", result.thread_id)
    print("turn_id:", result.turn_id)
    print("status:", result.status)
    print("error:", result.error)
    print("text:", result.text)
    print("items:", result.items)
    print("usage:", result.usage)
