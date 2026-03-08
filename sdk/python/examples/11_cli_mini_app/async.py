import asyncio

from codex_app_server import AsyncCodex, TextInput


async def main() -> None:
    print("Codex async mini CLI. Type /exit to quit.")

    async with AsyncCodex() as codex:
        thread = await codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})
        print("Thread:", thread.id)

        while True:
            try:
                user_input = (await asyncio.to_thread(input, "you> ")).strip()
            except EOFError:
                break

            if not user_input:
                continue
            if user_input in {"/exit", "/quit"}:
                break

            turn = await thread.turn(TextInput(user_input))
            result = await turn.run()

            if result.status == "failed":
                print("assistant> [failed]", result.error)
                continue

            print("assistant>", result.text.strip())


if __name__ == "__main__":
    asyncio.run(main())
