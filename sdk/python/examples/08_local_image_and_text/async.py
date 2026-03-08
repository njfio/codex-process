import asyncio
from pathlib import Path

from codex_app_server import AsyncCodex, LocalImageInput, TextInput

IMAGE_PATH = Path(__file__).resolve().parents[1] / "assets" / "sample_scene.png"
if not IMAGE_PATH.exists():
    raise FileNotFoundError(f"Missing bundled image: {IMAGE_PATH}")


async def main() -> None:
    async with AsyncCodex() as codex:
        thread = await codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})

        turn = await thread.turn(
            [
                TextInput("Read this local image and summarize what you see in 2 bullets."),
                LocalImageInput(str(IMAGE_PATH.resolve())),
            ]
        )
        result = await turn.run()

        print("Status:", result.status)
        print(result.text)


if __name__ == "__main__":
    asyncio.run(main())
