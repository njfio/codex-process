from codex_app_server import Codex, TextInput

print("Codex mini CLI. Type /exit to quit.")

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
    print("Thread:", thread.id)

    while True:
        try:
            user_input = input("you> ").strip()
        except EOFError:
            break

        if not user_input:
            continue
        if user_input in {"/exit", "/quit"}:
            break

        result = thread.turn(TextInput(user_input)).run()
        if result.status == "failed":
            print("assistant> [failed]", result.error)
            continue

        print("assistant>", result.text.strip())
