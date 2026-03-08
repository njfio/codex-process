from __future__ import annotations

from dataclasses import dataclass
from typing import AsyncIterator, Iterator

from .async_client import AsyncAppServerClient
from .client import AppServerClient, AppServerConfig
from .generated.v2_all.AgentMessageDeltaNotification import AgentMessageDeltaNotification
from .generated.v2_all.ThreadArchiveResponse import ThreadArchiveResponse
from .generated.v2_all.ThreadSetNameResponse import ThreadSetNameResponse
from .generated.v2_all.TurnCompletedNotification import TurnError
from .generated.v2_all.TurnInterruptResponse import TurnInterruptResponse
from .generated.v2_types import (
    ModelListResponse,
    ThreadCompactStartResponse,
    ThreadItem,
    ThreadListResponse,
    ThreadReadResponse,
    ThreadTokenUsageUpdatedNotification,
    TurnCompletedNotificationPayload,
    TurnSteerResponse,
)
from .models import InitializeResponse, JsonObject, Notification
from .public_types import (
    ForkAskForApproval,
    ForkSandboxMode,
    ResumeAskForApproval,
    ResumePersonality,
    ResumeSandboxMode,
    ThreadSortKey,
    ThreadSourceKind,
    TurnAskForApproval,
    TurnPersonality,
    TurnReasoningEffort,
    TurnReasoningSummary,
    TurnSandboxPolicy,
    AskForApproval,
    Personality,
    SandboxMode,
    ThreadForkParams,
    ThreadListParams,
    ThreadResumeParams,
    ThreadStartParams,
    TurnStartParams,
    TurnStatus,
)


@dataclass(slots=True)
class TurnResult:
    thread_id: str
    turn_id: str
    status: TurnStatus
    error: TurnError | None
    text: str
    items: list[ThreadItem]
    usage: ThreadTokenUsageUpdatedNotification | None = None


@dataclass(slots=True)
class TextInput:
    text: str


@dataclass(slots=True)
class ImageInput:
    url: str


@dataclass(slots=True)
class LocalImageInput:
    path: str


@dataclass(slots=True)
class SkillInput:
    name: str
    path: str


@dataclass(slots=True)
class MentionInput:
    name: str
    path: str


InputItem = TextInput | ImageInput | LocalImageInput | SkillInput | MentionInput
Input = list[InputItem] | InputItem


@dataclass(slots=True)
class InitializeResult:
    server_name: str | None = None
    server_version: str | None = None


def _to_wire_item(item: InputItem) -> JsonObject:
    if isinstance(item, TextInput):
        return {"type": "text", "text": item.text}
    if isinstance(item, ImageInput):
        return {"type": "image", "url": item.url}
    if isinstance(item, LocalImageInput):
        return {"type": "localImage", "path": item.path}
    if isinstance(item, SkillInput):
        return {"type": "skill", "name": item.name, "path": item.path}
    if isinstance(item, MentionInput):
        return {"type": "mention", "name": item.name, "path": item.path}
    raise TypeError(f"unsupported input item: {type(item)!r}")


def _to_wire_input(input: Input) -> list[JsonObject]:
    if isinstance(input, list):
        return [_to_wire_item(i) for i in input]
    return [_to_wire_item(input)]


class Codex:
    """Minimal typed SDK surface for app-server v2."""

    def __init__(self, config: AppServerConfig | None = None) -> None:
        self._client = AppServerClient(config=config)
        self._client.start()
        self._init = self._parse_initialize(self._client.initialize())

    def __enter__(self) -> "Codex":
        return self

    def __exit__(self, _exc_type, _exc, _tb) -> None:
        self.close()

    @staticmethod
    def _parse_initialize(payload: InitializeResponse) -> InitializeResult:
        server = payload.serverInfo
        if server is None:
            return InitializeResult()
        return InitializeResult(
            server_name=server.name,
            server_version=server.version,
        )

    @property
    def metadata(self) -> InitializeResult:
        return self._init

    def close(self) -> None:
        self._client.close()

    # BEGIN GENERATED: Codex.flat_methods
    def thread_start(
        self,
        *,
        approval_policy: AskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        ephemeral: bool | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: Personality | None = None,
        sandbox: SandboxMode | None = None,
    ) -> Thread:
        params = ThreadStartParams(
            approvalPolicy=approval_policy,
            baseInstructions=base_instructions,
            config=config,
            cwd=cwd,
            developerInstructions=developer_instructions,
            ephemeral=ephemeral,
            model=model,
            modelProvider=model_provider,
            personality=personality,
            sandbox=sandbox,
        )
        started = self._client.thread_start(params)
        return Thread(self._client, started.thread.id)

    def thread_list(
        self,
        *,
        archived: bool | None = None,
        cursor: str | None = None,
        cwd: str | None = None,
        limit: int | None = None,
        model_providers: list[str] | None = None,
        sort_key: ThreadSortKey | None = None,
        source_kinds: list[ThreadSourceKind] | None = None,
    ) -> ThreadListResponse:
        params = ThreadListParams(
            archived=archived,
            cursor=cursor,
            cwd=cwd,
            limit=limit,
            modelProviders=model_providers,
            sortKey=sort_key,
            sourceKinds=source_kinds,
        )
        return self._client.thread_list(params)

    def thread_resume(
        self,
        thread_id: str,
        *,
        approval_policy: ResumeAskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: ResumePersonality | None = None,
        sandbox: ResumeSandboxMode | None = None,
    ) -> Thread:
        params = ThreadResumeParams(
            threadId=thread_id,
            approvalPolicy=approval_policy,
            baseInstructions=base_instructions,
            config=config,
            cwd=cwd,
            developerInstructions=developer_instructions,
            model=model,
            modelProvider=model_provider,
            personality=personality,
            sandbox=sandbox,
        )
        resumed = self._client.thread_resume(thread_id, params)
        return Thread(self._client, resumed.thread.id)

    def thread_fork(
        self,
        thread_id: str,
        *,
        approval_policy: ForkAskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        sandbox: ForkSandboxMode | None = None,
    ) -> Thread:
        params = ThreadForkParams(
            threadId=thread_id,
            approvalPolicy=approval_policy,
            baseInstructions=base_instructions,
            config=config,
            cwd=cwd,
            developerInstructions=developer_instructions,
            model=model,
            modelProvider=model_provider,
            sandbox=sandbox,
        )
        forked = self._client.thread_fork(thread_id, params)
        return Thread(self._client, forked.thread.id)

    def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:
        return self._client.thread_archive(thread_id)

    def thread_unarchive(self, thread_id: str) -> Thread:
        unarchived = self._client.thread_unarchive(thread_id)
        return Thread(self._client, unarchived.thread.id)
    # END GENERATED: Codex.flat_methods

    def thread(self, thread_id: str) -> Thread:
        return Thread(self._client, thread_id)

    def models(self, *, include_hidden: bool = False) -> ModelListResponse:
        return self._client.model_list(include_hidden=include_hidden)


class AsyncCodex:
    """Async mirror of :class:`Codex` with matching method shapes."""

    def __init__(self, config: AppServerConfig | None = None) -> None:
        self._client = AsyncAppServerClient(config=config)
        self._init: InitializeResult | None = None
        self._initialized = False

    async def __aenter__(self) -> "AsyncCodex":
        await self._ensure_initialized()
        return self

    async def __aexit__(self, _exc_type, _exc, _tb) -> None:
        await self.close()

    async def _ensure_initialized(self) -> None:
        if self._initialized:
            return
        await self._client.start()
        payload = await self._client.initialize()
        self._init = Codex._parse_initialize(payload)
        self._initialized = True

    @property
    def metadata(self) -> InitializeResult:
        if self._init is None:
            raise RuntimeError(
                "AsyncCodex is not initialized yet. Use `async with AsyncCodex()` or call an async API first."
            )
        return self._init

    async def close(self) -> None:
        await self._client.close()
        self._init = None
        self._initialized = False

    # BEGIN GENERATED: AsyncCodex.flat_methods
    async def thread_start(
        self,
        *,
        approval_policy: AskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        ephemeral: bool | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: Personality | None = None,
        sandbox: SandboxMode | None = None,
    ) -> AsyncThread:
        await self._ensure_initialized()
        params = ThreadStartParams(
            approvalPolicy=approval_policy,
            baseInstructions=base_instructions,
            config=config,
            cwd=cwd,
            developerInstructions=developer_instructions,
            ephemeral=ephemeral,
            model=model,
            modelProvider=model_provider,
            personality=personality,
            sandbox=sandbox,
        )
        started = await self._client.thread_start(params)
        return AsyncThread(self, started.thread.id)

    async def thread_list(
        self,
        *,
        archived: bool | None = None,
        cursor: str | None = None,
        cwd: str | None = None,
        limit: int | None = None,
        model_providers: list[str] | None = None,
        sort_key: ThreadSortKey | None = None,
        source_kinds: list[ThreadSourceKind] | None = None,
    ) -> ThreadListResponse:
        await self._ensure_initialized()
        params = ThreadListParams(
            archived=archived,
            cursor=cursor,
            cwd=cwd,
            limit=limit,
            modelProviders=model_providers,
            sortKey=sort_key,
            sourceKinds=source_kinds,
        )
        return await self._client.thread_list(params)

    async def thread_resume(
        self,
        thread_id: str,
        *,
        approval_policy: ResumeAskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        personality: ResumePersonality | None = None,
        sandbox: ResumeSandboxMode | None = None,
    ) -> AsyncThread:
        await self._ensure_initialized()
        params = ThreadResumeParams(
            threadId=thread_id,
            approvalPolicy=approval_policy,
            baseInstructions=base_instructions,
            config=config,
            cwd=cwd,
            developerInstructions=developer_instructions,
            model=model,
            modelProvider=model_provider,
            personality=personality,
            sandbox=sandbox,
        )
        resumed = await self._client.thread_resume(thread_id, params)
        return AsyncThread(self, resumed.thread.id)

    async def thread_fork(
        self,
        thread_id: str,
        *,
        approval_policy: ForkAskForApproval | None = None,
        base_instructions: str | None = None,
        config: JsonObject | None = None,
        cwd: str | None = None,
        developer_instructions: str | None = None,
        model: str | None = None,
        model_provider: str | None = None,
        sandbox: ForkSandboxMode | None = None,
    ) -> AsyncThread:
        await self._ensure_initialized()
        params = ThreadForkParams(
            threadId=thread_id,
            approvalPolicy=approval_policy,
            baseInstructions=base_instructions,
            config=config,
            cwd=cwd,
            developerInstructions=developer_instructions,
            model=model,
            modelProvider=model_provider,
            sandbox=sandbox,
        )
        forked = await self._client.thread_fork(thread_id, params)
        return AsyncThread(self, forked.thread.id)

    async def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:
        await self._ensure_initialized()
        return await self._client.thread_archive(thread_id)

    async def thread_unarchive(self, thread_id: str) -> AsyncThread:
        await self._ensure_initialized()
        unarchived = await self._client.thread_unarchive(thread_id)
        return AsyncThread(self, unarchived.thread.id)
    # END GENERATED: AsyncCodex.flat_methods

    def thread(self, thread_id: str) -> AsyncThread:
        return AsyncThread(self, thread_id)

    async def models(self, *, include_hidden: bool = False) -> ModelListResponse:
        await self._ensure_initialized()
        return await self._client.model_list(include_hidden=include_hidden)


@dataclass(slots=True)
class Thread:
    _client: AppServerClient
    id: str

    # BEGIN GENERATED: Thread.flat_methods
    def turn(
        self,
        input: Input,
        *,
        approval_policy: TurnAskForApproval | None = None,
        cwd: str | None = None,
        effort: TurnReasoningEffort | None = None,
        model: str | None = None,
        output_schema: JsonObject | None = None,
        personality: TurnPersonality | None = None,
        sandbox_policy: TurnSandboxPolicy | None = None,
        summary: TurnReasoningSummary | None = None,
    ) -> Turn:
        wire_input = _to_wire_input(input)
        params = TurnStartParams(
            threadId=self.id,
            input=wire_input,
            approvalPolicy=approval_policy,
            cwd=cwd,
            effort=effort,
            model=model,
            outputSchema=output_schema,
            personality=personality,
            sandboxPolicy=sandbox_policy,
            summary=summary,
        )
        turn = self._client.turn_start(self.id, wire_input, params=params)
        return Turn(self._client, self.id, turn.turn.id)
    # END GENERATED: Thread.flat_methods

    def read(self, *, include_turns: bool = False) -> ThreadReadResponse:
        return self._client.thread_read(self.id, include_turns=include_turns)

    def set_name(self, name: str) -> ThreadSetNameResponse:
        return self._client.thread_set_name(self.id, name)

    def compact(self) -> ThreadCompactStartResponse:
        return self._client.thread_compact(self.id)


@dataclass(slots=True)
class AsyncThread:
    _codex: AsyncCodex
    id: str

    # BEGIN GENERATED: AsyncThread.flat_methods
    async def turn(
        self,
        input: Input,
        *,
        approval_policy: TurnAskForApproval | None = None,
        cwd: str | None = None,
        effort: TurnReasoningEffort | None = None,
        model: str | None = None,
        output_schema: JsonObject | None = None,
        personality: TurnPersonality | None = None,
        sandbox_policy: TurnSandboxPolicy | None = None,
        summary: TurnReasoningSummary | None = None,
    ) -> AsyncTurn:
        await self._codex._ensure_initialized()
        wire_input = _to_wire_input(input)
        params = TurnStartParams(
            threadId=self.id,
            input=wire_input,
            approvalPolicy=approval_policy,
            cwd=cwd,
            effort=effort,
            model=model,
            outputSchema=output_schema,
            personality=personality,
            sandboxPolicy=sandbox_policy,
            summary=summary,
        )
        turn = await self._codex._client.turn_start(
            self.id,
            wire_input,
            params=params,
        )
        return AsyncTurn(self._codex, self.id, turn.turn.id)
    # END GENERATED: AsyncThread.flat_methods

    async def read(self, *, include_turns: bool = False) -> ThreadReadResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.thread_read(self.id, include_turns=include_turns)

    async def set_name(self, name: str) -> ThreadSetNameResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.thread_set_name(self.id, name)

    async def compact(self) -> ThreadCompactStartResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.thread_compact(self.id)


@dataclass(slots=True)
class Turn:
    _client: AppServerClient
    thread_id: str
    id: str

    def steer(self, input: Input) -> TurnSteerResponse:
        return self._client.turn_steer(self.thread_id, self.id, _to_wire_input(input))

    def interrupt(self) -> TurnInterruptResponse:
        return self._client.turn_interrupt(self.thread_id, self.id)

    def stream(self) -> Iterator[Notification]:
        while True:
            event = self._client.next_notification()
            yield event
            if (
                event.method == "turn/completed"
                and isinstance(event.payload, TurnCompletedNotificationPayload)
                and event.payload.turn.id == self.id
            ):
                break

    def run(self) -> TurnResult:
        completed: TurnCompletedNotificationPayload | None = None
        usage: ThreadTokenUsageUpdatedNotification | None = None
        chunks: list[str] = []

        for event in self.stream():
            payload = event.payload
            if (
                isinstance(payload, AgentMessageDeltaNotification)
                and payload.turnId == self.id
            ):
                chunks.append(payload.delta)
                continue
            if (
                isinstance(payload, ThreadTokenUsageUpdatedNotification)
                and payload.turnId == self.id
            ):
                usage = payload
                continue
            if (
                isinstance(payload, TurnCompletedNotificationPayload)
                and payload.turn.id == self.id
            ):
                completed = payload

        if completed is None:
            raise RuntimeError("turn completed event not received")

        return TurnResult(
            thread_id=completed.threadId,
            turn_id=completed.turn.id,
            status=completed.turn.status,
            error=completed.turn.error,
            text="".join(chunks),
            items=list(completed.turn.items or []),
            usage=usage,
        )


@dataclass(slots=True)
class AsyncTurn:
    _codex: AsyncCodex
    thread_id: str
    id: str

    async def steer(self, input: Input) -> TurnSteerResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.turn_steer(
            self.thread_id,
            self.id,
            _to_wire_input(input),
        )

    async def interrupt(self) -> TurnInterruptResponse:
        await self._codex._ensure_initialized()
        return await self._codex._client.turn_interrupt(self.thread_id, self.id)

    async def stream(self) -> AsyncIterator[Notification]:
        await self._codex._ensure_initialized()
        while True:
            event = await self._codex._client.next_notification()
            yield event
            if (
                event.method == "turn/completed"
                and isinstance(event.payload, TurnCompletedNotificationPayload)
                and event.payload.turn.id == self.id
            ):
                break

    async def run(self) -> TurnResult:
        completed: TurnCompletedNotificationPayload | None = None
        usage: ThreadTokenUsageUpdatedNotification | None = None
        chunks: list[str] = []

        async for event in self.stream():
            payload = event.payload
            if (
                isinstance(payload, AgentMessageDeltaNotification)
                and payload.turnId == self.id
            ):
                chunks.append(payload.delta)
                continue
            if (
                isinstance(payload, ThreadTokenUsageUpdatedNotification)
                and payload.turnId == self.id
            ):
                usage = payload
                continue
            if (
                isinstance(payload, TurnCompletedNotificationPayload)
                and payload.turn.id == self.id
            ):
                completed = payload

        if completed is None:
            raise RuntimeError("turn completed event not received")

        return TurnResult(
            thread_id=completed.threadId,
            turn_id=completed.turn.id,
            status=completed.turn.status,
            error=completed.turn.error,
            text="".join(chunks),
            items=list(completed.turn.items or []),
            usage=usage,
        )
