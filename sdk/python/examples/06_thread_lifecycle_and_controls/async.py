import asyncio

from codex_app_server import AsyncCodex


async def main() -> None:
    async with AsyncCodex() as codex:
        thread = await codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})

        await codex.thread_list(limit=20)
        await thread.read(include_turns=False)

        print("Lifecycle OK:", thread.id)


if __name__ == "__main__":
    asyncio.run(main())
