from __future__ import annotations

import asyncio
from typing import Callable, Iterable, ParamSpec, TypeVar

from pydantic import BaseModel

from .client import AppServerClient, AppServerConfig
from .conversation import AsyncThreadSession
from .generated.v2_all.AgentMessageDeltaNotification import AgentMessageDeltaNotification
from .generated.v2_all.ModelListResponse import ModelListResponse
from .generated.v2_all.ThreadArchiveResponse import ThreadArchiveResponse
from .generated.v2_all.ThreadCompactStartResponse import ThreadCompactStartResponse
from .generated.v2_all.ThreadForkParams import ThreadForkParams as V2ThreadForkParams
from .generated.v2_all.ThreadForkResponse import ThreadForkResponse
from .generated.v2_all.ThreadListParams import ThreadListParams as V2ThreadListParams
from .generated.v2_all.ThreadListResponse import ThreadListResponse
from .generated.v2_all.ThreadReadResponse import ThreadReadResponse
from .generated.v2_all.ThreadResumeParams import ThreadResumeParams as V2ThreadResumeParams
from .generated.v2_all.ThreadResumeResponse import ThreadResumeResponse
from .generated.v2_all.ThreadSetNameResponse import ThreadSetNameResponse
from .generated.v2_all.ThreadStartParams import ThreadStartParams as V2ThreadStartParams
from .generated.v2_all.ThreadStartResponse import ThreadStartResponse
from .generated.v2_all.ThreadUnarchiveResponse import ThreadUnarchiveResponse
from .generated.v2_all.TurnCompletedNotification import TurnCompletedNotification
from .generated.v2_all.TurnInterruptResponse import TurnInterruptResponse
from .generated.v2_all.TurnStartParams import TurnStartParams as V2TurnStartParams
from .generated.v2_all.TurnStartResponse import TurnStartResponse
from .generated.v2_all.TurnSteerResponse import TurnSteerResponse
from .models import InitializeResponse, JsonObject, Notification, TextTurnResult

ModelT = TypeVar("ModelT", bound=BaseModel)
ParamsT = ParamSpec("ParamsT")
ReturnT = TypeVar("ReturnT")


class AsyncAppServerClient:
    """Async wrapper around AppServerClient using thread offloading."""

    def __init__(self, config: AppServerConfig | None = None):
        self._sync = AppServerClient(config=config)

    async def __aenter__(self) -> "AsyncAppServerClient":
        await self.start()
        return self

    async def __aexit__(self, _exc_type, _exc, _tb) -> None:
        await self.close()

    async def _call_sync(
        self,
        fn: Callable[ParamsT, ReturnT],
        /,
        *args: ParamsT.args,
        **kwargs: ParamsT.kwargs,
    ) -> ReturnT:
        return await asyncio.to_thread(fn, *args, **kwargs)

    async def start(self) -> None:
        await self._call_sync(self._sync.start)

    async def close(self) -> None:
        await self._call_sync(self._sync.close)

    async def initialize(self) -> InitializeResponse:
        return await self._call_sync(self._sync.initialize)

    async def request(
        self,
        method: str,
        params: JsonObject | None,
        *,
        response_model: type[ModelT],
    ) -> ModelT:
        return await self._call_sync(
            self._sync.request,
            method,
            params,
            response_model=response_model,
        )

    async def thread_start(self, params: V2ThreadStartParams | JsonObject | None = None) -> ThreadStartResponse:
        return await self._call_sync(self._sync.thread_start, params)

    async def thread_resume(
        self,
        thread_id: str,
        params: V2ThreadResumeParams | JsonObject | None = None,
    ) -> ThreadResumeResponse:
        return await self._call_sync(self._sync.thread_resume, thread_id, params)

    async def thread_list(self, params: V2ThreadListParams | JsonObject | None = None) -> ThreadListResponse:
        return await self._call_sync(self._sync.thread_list, params)

    async def thread_read(self, thread_id: str, include_turns: bool = False) -> ThreadReadResponse:
        return await self._call_sync(self._sync.thread_read, thread_id, include_turns)

    async def thread_fork(
        self,
        thread_id: str,
        params: V2ThreadForkParams | JsonObject | None = None,
    ) -> ThreadForkResponse:
        return await self._call_sync(self._sync.thread_fork, thread_id, params)

    async def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:
        return await self._call_sync(self._sync.thread_archive, thread_id)

    async def thread_unarchive(self, thread_id: str) -> ThreadUnarchiveResponse:
        return await self._call_sync(self._sync.thread_unarchive, thread_id)

    async def thread_set_name(self, thread_id: str, name: str) -> ThreadSetNameResponse:
        return await self._call_sync(self._sync.thread_set_name, thread_id, name)

    async def thread_compact(self, thread_id: str) -> ThreadCompactStartResponse:
        return await self._call_sync(self._sync.thread_compact, thread_id)

    async def turn_start(
        self,
        thread_id: str,
        input_items: list[JsonObject] | JsonObject | str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TurnStartResponse:
        return await self._call_sync(self._sync.turn_start, thread_id, input_items, params)

    async def turn_text(
        self,
        thread_id: str,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TurnStartResponse:
        return await self._call_sync(self._sync.turn_text, thread_id, text, params)

    async def turn_interrupt(self, thread_id: str, turn_id: str) -> TurnInterruptResponse:
        return await self._call_sync(self._sync.turn_interrupt, thread_id, turn_id)

    async def turn_steer(
        self,
        thread_id: str,
        expected_turn_id: str,
        input_items: list[JsonObject] | JsonObject | str,
    ) -> TurnSteerResponse:
        return await self._call_sync(
            self._sync.turn_steer,
            thread_id,
            expected_turn_id,
            input_items,
        )

    async def model_list(self, include_hidden: bool = False) -> ModelListResponse:
        return await self._call_sync(self._sync.model_list, include_hidden)

    def thread(self, thread_id: str) -> AsyncThreadSession:
        return AsyncThreadSession(client=self, thread_id=thread_id)

    async def thread_start_session(
        self,
        *,
        model: str | None = None,
        params: V2ThreadStartParams | JsonObject | None = None,
    ) -> AsyncThreadSession:
        sync_session = await self._call_sync(
            self._sync.thread_start_session,
            model=model,
            params=params,
        )
        return AsyncThreadSession(client=self, thread_id=sync_session.thread_id)

    async def request_with_retry_on_overload(
        self,
        method: str,
        params: JsonObject | None,
        *,
        response_model: type[ModelT],
        max_attempts: int = 3,
        initial_delay_s: float = 0.25,
        max_delay_s: float = 2.0,
    ) -> ModelT:
        return await self._call_sync(
            self._sync.request_with_retry_on_overload,
            method,
            params,
            response_model=response_model,
            max_attempts=max_attempts,
            initial_delay_s=initial_delay_s,
            max_delay_s=max_delay_s,
        )

    async def next_notification(self) -> Notification:
        return await self._call_sync(self._sync.next_notification)

    async def wait_for_turn_completed(self, turn_id: str) -> TurnCompletedNotification:
        return await self._call_sync(self._sync.wait_for_turn_completed, turn_id)

    async def stream_until_methods(self, methods: Iterable[str] | str) -> list[Notification]:
        return await self._call_sync(self._sync.stream_until_methods, methods)

    async def run_text_turn(
        self,
        thread_id: str,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TextTurnResult:
        return await self._call_sync(self._sync.run_text_turn, thread_id, text, params)

    async def ask_result(
        self,
        text: str,
        *,
        model: str | None = None,
        thread_id: str | None = None,
    ) -> TextTurnResult:
        return await self._call_sync(
            self._sync.ask_result,
            text,
            model=model,
            thread_id=thread_id,
        )

    async def ask(
        self,
        text: str,
        *,
        model: str | None = None,
        thread_id: str | None = None,
    ) -> TextTurnResult:
        return await self._call_sync(
            self._sync.ask,
            text,
            model=model,
            thread_id=thread_id,
        )

    async def stream_text(
        self,
        thread_id: str,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> list[AgentMessageDeltaNotification]:
        return await self._call_sync(
            lambda: list(self._sync.stream_text(thread_id, text, params))
        )
