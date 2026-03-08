from codex_app_server import Codex, TextInput

with Codex() as codex:
    # Create an initial thread and turn so we have a real thread to resume.
    original = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
    first = original.turn(TextInput("Tell me one fact about Saturn.")).run()
    print("Created thread:", first.thread_id)

    # Resume the existing thread by ID.
    resumed = codex.thread(first.thread_id)
    second = resumed.turn(TextInput("Continue with one more fact.")).run()
    print(second.text)
