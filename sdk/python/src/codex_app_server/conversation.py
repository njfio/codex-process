from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, AsyncIterator, Iterator

from .generated.v2_all.AgentMessageDeltaNotification import AgentMessageDeltaNotification
from .generated.v2_all.TurnCompletedNotification import TurnCompletedNotification
from .generated.v2_all.TurnStartParams import TurnStartParams as V2TurnStartParams
from .generated.v2_all.TurnStartResponse import TurnStartResponse
from .generated.v2_all.TurnSteerResponse import TurnSteerResponse
from .models import JsonObject, Notification, TextTurnResult

if TYPE_CHECKING:
    from .async_client import AsyncAppServerClient
    from .client import AppServerClient


@dataclass(slots=True)
class ThreadSession:
    """Fluent thread-scoped helper over :class:`AppServerClient`."""

    client: "AppServerClient"
    thread_id: str

    def turn_start(
        self,
        input_items: list[JsonObject] | JsonObject | str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TurnStartResponse:
        return self.client.turn_start(self.thread_id, input_items, params)

    def turn_text(
        self,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TurnStartResponse:
        return self.client.turn_text(self.thread_id, text, params)

    def turn_steer(
        self,
        expected_turn_id: str,
        input_items: list[JsonObject] | JsonObject | str,
    ) -> TurnSteerResponse:
        return self.client.turn_steer(self.thread_id, expected_turn_id, input_items)

    def ask_result(
        self,
        text: str,
        *,
        model: str | None = None,
    ) -> TextTurnResult:
        return self.client.ask_result(text, model=model, thread_id=self.thread_id)

    def ask(
        self,
        text: str,
        *,
        model: str | None = None,
    ) -> TextTurnResult:
        return self.client.ask(text, model=model, thread_id=self.thread_id)

    def stream_text(
        self,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> Iterator[AgentMessageDeltaNotification]:
        yield from self.client.stream_text(self.thread_id, text, params)

    def stream(
        self,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> Iterator[Notification]:
        turn = self.turn_text(text, params)
        turn_id = turn.turn.id
        while True:
            event = self.client.next_notification()
            yield event
            if (
                event.method == "turn/completed"
                and isinstance(event.payload, TurnCompletedNotification)
                and event.payload.turn.id == turn_id
            ):
                break


@dataclass(slots=True)
class AsyncThreadSession:
    """Fluent thread-scoped helper over :class:`AsyncAppServerClient`."""

    client: "AsyncAppServerClient"
    thread_id: str

    async def turn_start(
        self,
        input_items: list[JsonObject] | JsonObject | str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TurnStartResponse:
        return await self.client.turn_start(self.thread_id, input_items, params)

    async def turn_text(
        self,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TurnStartResponse:
        return await self.client.turn_text(self.thread_id, text, params)

    async def turn_steer(
        self,
        expected_turn_id: str,
        input_items: list[JsonObject] | JsonObject | str,
    ) -> TurnSteerResponse:
        return await self.client.turn_steer(self.thread_id, expected_turn_id, input_items)

    async def ask_result(
        self,
        text: str,
        *,
        model: str | None = None,
    ) -> TextTurnResult:
        return await self.client.ask_result(text, model=model, thread_id=self.thread_id)

    async def ask(
        self,
        text: str,
        *,
        model: str | None = None,
    ) -> TextTurnResult:
        return await self.client.ask(text, model=model, thread_id=self.thread_id)

    async def stream_text(
        self,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> AsyncIterator[AgentMessageDeltaNotification]:
        for chunk in await self.client.stream_text(self.thread_id, text, params):
            yield chunk

    async def stream(
        self,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> AsyncIterator[Notification]:
        turn = await self.turn_text(text, params)
        turn_id = turn.turn.id
        while True:
            event = await self.client.next_notification()
            yield event
            if (
                event.method == "turn/completed"
                and isinstance(event.payload, TurnCompletedNotification)
                and event.payload.turn.id == turn_id
            ):
                break


# Backward-compatible aliases (v2 prefers thread terminology).
Conversation = ThreadSession
AsyncConversation = AsyncThreadSession
