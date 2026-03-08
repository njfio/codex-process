import asyncio

from codex_app_server import AsyncCodex, ImageInput, TextInput

REMOTE_IMAGE_URL = "https://github.githubassets.com/images/modules/logos_page/GitHub-Mark.png"


async def main() -> None:
    async with AsyncCodex() as codex:
        thread = await codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
        turn = await thread.turn(
            [
                TextInput("What is in this image? Give 3 bullets."),
                ImageInput(REMOTE_IMAGE_URL),
            ]
        )
        result = await turn.run()

        print("Status:", result.status)
        print(result.text)


if __name__ == "__main__":
    asyncio.run(main())
