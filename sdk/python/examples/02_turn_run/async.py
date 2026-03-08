import asyncio

from codex_app_server import AsyncCodex, TextInput


async def main() -> None:
    async with AsyncCodex() as codex:
        thread = await codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
        turn = await thread.turn(TextInput("Give 3 bullets about SIMD."))
        result = await turn.run()

        print("thread_id:", result.thread_id)
        print("turn_id:", result.turn_id)
        print("status:", result.status)
        print("error:", result.error)
        print("text:", result.text)
        print("items:", result.items)
        print("usage:", result.usage)


if __name__ == "__main__":
    asyncio.run(main())
