import asyncio
from base64 import b64decode
from pathlib import Path

from codex_app_server import AsyncCodex, LocalImageInput, TextInput

HERE = Path(__file__).parent
IMAGE_PATH = HERE / "sample.png"

if not IMAGE_PATH.exists():
    IMAGE_PATH.write_bytes(
        b64decode(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO7Z4xQAAAAASUVORK5CYII="
        )
    )


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
