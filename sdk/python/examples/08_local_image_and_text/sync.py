from pathlib import Path

from codex_app_server import Codex, LocalImageInput, TextInput

IMAGE_PATH = Path(__file__).resolve().parents[1] / "assets" / "sample_scene.png"
if not IMAGE_PATH.exists():
    raise FileNotFoundError(f"Missing bundled image: {IMAGE_PATH}")

with Codex() as codex:
    thread = codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})

    result = thread.turn(
        [
            TextInput("Read this local image and summarize what you see in 2 bullets."),
            LocalImageInput(str(IMAGE_PATH.resolve())),
        ]
    ).run()

    print("Status:", result.status)
    print(result.text)
