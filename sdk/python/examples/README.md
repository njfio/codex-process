# Python SDK Examples

Each example folder contains runnable versions:

- `sync.py` (public sync surface: `Codex`)
- `async.py` (public async surface: `AsyncCodex`)

All examples intentionally use only public SDK exports from `codex_app_server`.

## Run format

From `sdk/python`:

```bash
python examples/<example-folder>/sync.py
python examples/<example-folder>/async.py
```

## Recommended first run

```bash
python examples/01_quickstart_constructor/sync.py
python examples/01_quickstart_constructor/async.py
```

## Index

- `01_quickstart_constructor/`
  - first run / sanity check
- `02_turn_run/`
  - inspect full turn output fields
- `03_turn_stream_events/`
  - stream and print raw notifications
- `04_models_and_metadata/`
  - read server metadata and model list
- `05_existing_thread/`
  - resume a real existing thread (created in-script)
- `06_thread_lifecycle_and_controls/`
  - thread lifecycle + control calls
- `07_image_and_text/`
  - remote image URL + text multimodal turn
- `08_local_image_and_text/`
  - local image + text multimodal turn using bundled sample image
- `09_async_parity/`
  - parity-style sync flow (see async parity in other examples)
- `10_error_handling_and_retry/`
  - overload retry pattern + typed error handling structure
- `11_cli_mini_app/`
  - interactive chat loop
- `12_turn_params_kitchen_sink/`
  - one turn using most optional `turn(...)` params (sync + async)
