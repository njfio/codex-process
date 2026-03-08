import asyncio

from codex_app_server import AsyncCodex, TextInput


async def main() -> None:
    async with AsyncCodex() as codex:
        thread = await codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
        turn = await thread.turn(TextInput("Write a short haiku about compilers."))

        async for event in turn.stream():
            print(event.method, event.payload)


if __name__ == "__main__":
    asyncio.run(main())
