from codex_app_server import Codex, ImageInput, TextInput

REMOTE_IMAGE_URL = "https://github.githubassets.com/images/modules/logos_page/GitHub-Mark.png"

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
    result = thread.turn(
        [
            TextInput("What is in this image? Give 3 bullets."),
            ImageInput(REMOTE_IMAGE_URL),
        ]
    ).run()

    print("Status:", result.status)
    print(result.text)
