from __future__ import annotations

from typing import Any

from codex_app_server.client import AppServerClient
from codex_app_server.generated.codex_event_types import CodexEventNotification
from codex_app_server.generated.v2_all.ThreadTokenUsageUpdatedNotification import (
    ThreadTokenUsageUpdatedNotification,
)
from codex_app_server.models import UnknownNotification


def test_thread_set_name_and_compact_use_current_rpc_methods() -> None:
    client = AppServerClient()
    calls: list[tuple[str, dict[str, Any] | None]] = []

    def fake_request(method: str, params, *, response_model):  # type: ignore[no-untyped-def]
        calls.append((method, params))
        return response_model.model_validate({})

    client.request = fake_request  # type: ignore[method-assign]

    client.thread_set_name("thread-1", "sdk-name")
    client.thread_compact("thread-1")

    assert calls[0][0] == "thread/name/set"
    assert calls[1][0] == "thread/compact/start"


def test_notification_aliases_are_canonicalized_and_typed() -> None:
    client = AppServerClient()
    event = client._coerce_notification(
        "thread/tokenUsageUpdated",
        {
            "threadId": "thread-1",
            "turnId": "turn-1",
            "tokenUsage": {
                "last": {
                    "cachedInputTokens": 0,
                    "inputTokens": 1,
                    "outputTokens": 2,
                    "reasoningOutputTokens": 0,
                    "totalTokens": 3,
                },
                "total": {
                    "cachedInputTokens": 0,
                    "inputTokens": 1,
                    "outputTokens": 2,
                    "reasoningOutputTokens": 0,
                    "totalTokens": 3,
                },
            },
        },
    )

    assert event.method == "thread/tokenUsage/updated"
    assert isinstance(event.payload, ThreadTokenUsageUpdatedNotification)
    assert event.payload.turnId == "turn-1"


def test_codex_event_notifications_are_typed() -> None:
    client = AppServerClient()
    event = client._coerce_notification(
        "codex/event/turn_aborted",
        {
            "id": "evt-1",
            "conversationId": "thread-1",
            "msg": {"type": "turn_aborted"},
        },
    )

    assert event.method == "codex/event/turn_aborted"
    assert isinstance(event.payload, CodexEventNotification)
    assert event.payload.msg.type == "turn_aborted"


def test_invalid_notification_payload_falls_back_to_unknown() -> None:
    client = AppServerClient()
    event = client._coerce_notification("thread/tokenUsage/updated", {"threadId": "missing"})

    assert event.method == "thread/tokenUsage/updated"
    assert isinstance(event.payload, UnknownNotification)
