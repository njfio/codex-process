import asyncio

from codex_app_server import AsyncCodex, TextInput


async def main() -> None:
    async with AsyncCodex() as codex:
        print("Server:", codex.metadata.server_name, codex.metadata.server_version)

        thread = await codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
        turn = await thread.turn(TextInput("Say hello in one sentence."))
        result = await turn.run()

        print("Status:", result.status)
        print("Text:", result.text)


if __name__ == "__main__":
    asyncio.run(main())
