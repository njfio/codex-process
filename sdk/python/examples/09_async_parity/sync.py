from codex_app_server import Codex, TextInput

with Codex() as codex:
    print("Server:", codex.metadata.server_name, codex.metadata.server_version)

    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
    turn = thread.turn(TextInput("Say hello in one sentence."))
    result = turn.run()

    print("Thread:", result.thread_id)
    print("Turn:", result.turn_id)
    print("Text:", result.text.strip())
