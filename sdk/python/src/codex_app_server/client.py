from __future__ import annotations

import json
import os
import subprocess
import threading
import uuid
from collections import deque
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable, Iterator, TypeVar

from pydantic import BaseModel

from .conversation import ThreadSession
from .errors import AppServerError, TransportClosedError, map_jsonrpc_error
from .generated.v2_all.AccountLoginCompletedNotification import (
    AccountLoginCompletedNotification,
)
from .generated.v2_all.AccountRateLimitsUpdatedNotification import (
    AccountRateLimitsUpdatedNotification,
)
from .generated.v2_all.AccountUpdatedNotification import AccountUpdatedNotification
from .generated.v2_all.AgentMessageDeltaNotification import AgentMessageDeltaNotification
from .generated.v2_all.AppListUpdatedNotification import AppListUpdatedNotification
from .generated.v2_all.CommandExecutionOutputDeltaNotification import (
    CommandExecutionOutputDeltaNotification,
)
from .generated.v2_all.ConfigWarningNotification import ConfigWarningNotification
from .generated.v2_all.ContextCompactedNotification import ContextCompactedNotification
from .generated.v2_all.DeprecationNoticeNotification import DeprecationNoticeNotification
from .generated.v2_all.ErrorNotification import ErrorNotification
from .generated.v2_all.FileChangeOutputDeltaNotification import (
    FileChangeOutputDeltaNotification,
)
from .generated.v2_all.ItemCompletedNotification import ItemCompletedNotification
from .generated.v2_all.ItemStartedNotification import ItemStartedNotification
from .generated.v2_all.McpServerOauthLoginCompletedNotification import (
    McpServerOauthLoginCompletedNotification,
)
from .generated.v2_all.McpToolCallProgressNotification import McpToolCallProgressNotification
from .generated.v2_all.ModelListResponse import ModelListResponse
from .generated.v2_all.PlanDeltaNotification import PlanDeltaNotification
from .generated.v2_all.RawResponseItemCompletedNotification import (
    RawResponseItemCompletedNotification,
)
from .generated.v2_all.ReasoningSummaryPartAddedNotification import (
    ReasoningSummaryPartAddedNotification,
)
from .generated.v2_all.ReasoningSummaryTextDeltaNotification import (
    ReasoningSummaryTextDeltaNotification,
)
from .generated.v2_all.ReasoningTextDeltaNotification import ReasoningTextDeltaNotification
from .generated.v2_all.TerminalInteractionNotification import TerminalInteractionNotification
from .generated.v2_all.ThreadArchiveResponse import ThreadArchiveResponse
from .generated.v2_all.ThreadCompactStartResponse import ThreadCompactStartResponse
from .generated.v2_all.ThreadForkParams import ThreadForkParams as V2ThreadForkParams
from .generated.v2_all.ThreadForkResponse import ThreadForkResponse
from .generated.v2_all.ThreadListParams import ThreadListParams as V2ThreadListParams
from .generated.v2_all.ThreadListResponse import ThreadListResponse
from .generated.v2_all.ThreadNameUpdatedNotification import ThreadNameUpdatedNotification
from .generated.v2_all.ThreadReadResponse import ThreadReadResponse
from .generated.v2_all.ThreadResumeParams import ThreadResumeParams as V2ThreadResumeParams
from .generated.v2_all.ThreadResumeResponse import ThreadResumeResponse
from .generated.v2_all.ThreadSetNameResponse import ThreadSetNameResponse
from .generated.v2_all.ThreadStartParams import ThreadStartParams as V2ThreadStartParams
from .generated.v2_all.ThreadStartResponse import ThreadStartResponse
from .generated.v2_all.ThreadStartedNotification import ThreadStartedNotification
from .generated.v2_all.ThreadTokenUsageUpdatedNotification import (
    ThreadTokenUsageUpdatedNotification,
)
from .generated.v2_all.ThreadUnarchiveResponse import ThreadUnarchiveResponse
from .generated.v2_all.TurnCompletedNotification import TurnCompletedNotification
from .generated.v2_all.TurnDiffUpdatedNotification import TurnDiffUpdatedNotification
from .generated.v2_all.TurnInterruptResponse import TurnInterruptResponse
from .generated.v2_all.TurnPlanUpdatedNotification import TurnPlanUpdatedNotification
from .generated.v2_all.TurnStartParams import TurnStartParams as V2TurnStartParams
from .generated.v2_all.TurnStartResponse import TurnStartResponse
from .generated.v2_all.TurnStartedNotification import TurnStartedNotification
from .generated.v2_all.TurnSteerResponse import TurnSteerResponse
from .generated.v2_all.WindowsWorldWritableWarningNotification import (
    WindowsWorldWritableWarningNotification,
)
from .models import (
    InitializeResponse,
    JsonObject,
    JsonValue,
    Notification,
    TextTurnResult,
)
from .retry import retry_on_overload

ModelT = TypeVar("ModelT", bound=BaseModel)
ApprovalHandler = Callable[[str, JsonObject | None], JsonObject]


def _params_dict(params: V2ThreadStartParams | V2ThreadResumeParams | V2ThreadListParams | V2ThreadForkParams | V2TurnStartParams | JsonObject | None) -> JsonObject:
    if params is None:
        return {}
    if hasattr(params, "model_dump"):
        dumped = params.model_dump(exclude_none=True, mode="json")
        if not isinstance(dumped, dict):
            raise TypeError("Expected model_dump() to return dict")
        return dumped
    if isinstance(params, dict):
        return params
    raise TypeError(f"Expected generated params model or dict, got {type(params).__name__}")


_NOTIFICATION_MODELS: dict[str, type[BaseModel]] = {
    "account/loginCompleted": AccountLoginCompletedNotification,
    "account/rateLimitsUpdated": AccountRateLimitsUpdatedNotification,
    "account/updated": AccountUpdatedNotification,
    "app/listUpdated": AppListUpdatedNotification,
    "commandExecution/outputDelta": CommandExecutionOutputDeltaNotification,
    "config/warning": ConfigWarningNotification,
    "context/compacted": ContextCompactedNotification,
    "deprecationNotice": DeprecationNoticeNotification,
    "error": ErrorNotification,
    "fileChange/outputDelta": FileChangeOutputDeltaNotification,
    "item/agentMessage/delta": AgentMessageDeltaNotification,
    "item/completed": ItemCompletedNotification,
    "item/started": ItemStartedNotification,
    "mcp/serverOauthLoginCompleted": McpServerOauthLoginCompletedNotification,
    "mcp/toolCallProgress": McpToolCallProgressNotification,
    "plan/delta": PlanDeltaNotification,
    "rawResponseItem/completed": RawResponseItemCompletedNotification,
    "reasoning/summaryPartAdded": ReasoningSummaryPartAddedNotification,
    "reasoning/summaryTextDelta": ReasoningSummaryTextDeltaNotification,
    "reasoning/textDelta": ReasoningTextDeltaNotification,
    "terminal/interaction": TerminalInteractionNotification,
    "thread/nameUpdated": ThreadNameUpdatedNotification,
    "thread/started": ThreadStartedNotification,
    "thread/tokenUsageUpdated": ThreadTokenUsageUpdatedNotification,
    "turn/completed": TurnCompletedNotification,
    "turn/diffUpdated": TurnDiffUpdatedNotification,
    "turn/planUpdated": TurnPlanUpdatedNotification,
    "turn/started": TurnStartedNotification,
    "windows/worldWritableWarning": WindowsWorldWritableWarningNotification,
}


def _bundled_codex_path() -> Path:
    import platform

    sys_name = platform.system().lower()
    machine = platform.machine().lower()

    if sys_name.startswith("darwin"):
        platform_dir = "darwin-arm64" if machine in {"arm64", "aarch64"} else "darwin-x64"
        exe = "codex"
    elif sys_name.startswith("linux"):
        platform_dir = "linux-arm64" if machine in {"arm64", "aarch64"} else "linux-x64"
        exe = "codex"
    elif sys_name.startswith("windows") or os.name == "nt":
        platform_dir = "windows-arm64" if machine in {"arm64", "aarch64"} else "windows-x64"
        exe = "codex.exe"
    else:
        raise RuntimeError(f"Unsupported OS for bundled codex binary: {sys_name}/{machine}")

    return Path(__file__).resolve().parent / "bin" / platform_dir / exe


@dataclass(slots=True)
class AppServerConfig:
    codex_bin: str = str(_bundled_codex_path())
    launch_args_override: tuple[str, ...] | None = None
    config_overrides: tuple[str, ...] = ()
    cwd: str | None = None
    env: dict[str, str] | None = None
    client_name: str = "codex_python_sdk"
    client_title: str = "Codex Python SDK"
    client_version: str = "0.2.0"
    experimental_api: bool = True


class AppServerClient:
    """Synchronous typed JSON-RPC client for `codex app-server` over stdio."""

    def __init__(
        self,
        config: AppServerConfig | None = None,
        approval_handler: ApprovalHandler | None = None,
    ) -> None:
        self.config = config or AppServerConfig()
        self._approval_handler = approval_handler or self._default_approval_handler
        self._proc: subprocess.Popen[str] | None = None
        self._lock = threading.Lock()
        self._pending_notifications: deque[Notification] = deque()
        self._stderr_lines: deque[str] = deque(maxlen=400)
        self._stderr_thread: threading.Thread | None = None

    def __enter__(self) -> "AppServerClient":
        self.start()
        return self

    def __exit__(self, _exc_type, _exc, _tb) -> None:
        self.close()

    def start(self) -> None:
        if self._proc is not None:
            return

        if self.config.launch_args_override is not None:
            args = list(self.config.launch_args_override)
        else:
            codex_bin = Path(self.config.codex_bin)
            if not codex_bin.exists():
                raise FileNotFoundError(
                    f"Pinned codex binary not found at {codex_bin}. Run `python scripts/update_sdk_artifacts.py --channel stable` from sdk/python."
                )
            args = [str(codex_bin)]
            for kv in self.config.config_overrides:
                args.extend(["--config", kv])
            args.extend(["app-server", "--listen", "stdio://"])

        env = os.environ.copy()
        if self.config.env:
            env.update(self.config.env)

        self._proc = subprocess.Popen(
            args,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            cwd=self.config.cwd,
            env=env,
            bufsize=1,
        )

        self._start_stderr_drain_thread()

    def close(self) -> None:
        if self._proc is None:
            return
        proc = self._proc
        self._proc = None

        if proc.stdin:
            proc.stdin.close()
        try:
            proc.terminate()
            proc.wait(timeout=2)
        except Exception:
            proc.kill()

        if self._stderr_thread and self._stderr_thread.is_alive():
            self._stderr_thread.join(timeout=0.5)

    def initialize(self) -> InitializeResponse:
        result = self.request(
            "initialize",
            {
                "clientInfo": {
                    "name": self.config.client_name,
                    "title": self.config.client_title,
                    "version": self.config.client_version,
                },
                "capabilities": {
                    "experimentalApi": self.config.experimental_api,
                },
            },
            response_model=InitializeResponse,
        )
        self.notify("initialized", None)
        return result

    def request(
        self,
        method: str,
        params: JsonObject | None,
        *,
        response_model: type[ModelT],
    ) -> ModelT:
        result = self._request_raw(method, params)
        if not isinstance(result, dict):
            raise AppServerError(f"{method} response must be a JSON object")
        return response_model.model_validate(result)

    def _request_raw(self, method: str, params: JsonObject | None = None) -> JsonValue:
        request_id = str(uuid.uuid4())
        self._write_message({"id": request_id, "method": method, "params": params or {}})

        while True:
            msg = self._read_message()

            if "method" in msg and "id" in msg:
                response = self._handle_server_request(msg)
                self._write_message({"id": msg["id"], "result": response})
                continue

            if "method" in msg and "id" not in msg:
                self._pending_notifications.append(self._coerce_notification(msg["method"], msg.get("params")))
                continue

            if msg.get("id") != request_id:
                continue

            if "error" in msg:
                err = msg["error"]
                if isinstance(err, dict):
                    raise map_jsonrpc_error(
                        int(err.get("code", -32000)),
                        str(err.get("message", "unknown")),
                        err.get("data"),
                    )
                raise AppServerError("Malformed JSON-RPC error response")

            return msg.get("result")

    def notify(self, method: str, params: JsonObject | None = None) -> None:
        self._write_message({"method": method, "params": params or {}})

    def next_notification(self) -> Notification:
        if self._pending_notifications:
            return self._pending_notifications.popleft()

        while True:
            msg = self._read_message()
            if "method" in msg and "id" in msg:
                response = self._handle_server_request(msg)
                self._write_message({"id": msg["id"], "result": response})
                continue
            if "method" in msg and "id" not in msg:
                return self._coerce_notification(msg["method"], msg.get("params"))

    def thread_start(self, params: V2ThreadStartParams | JsonObject | None = None) -> ThreadStartResponse:
        return self.request("thread/start", _params_dict(params), response_model=ThreadStartResponse)

    def thread_resume(
        self,
        thread_id: str,
        params: V2ThreadResumeParams | JsonObject | None = None,
    ) -> ThreadResumeResponse:
        payload = {"threadId": thread_id, **_params_dict(params)}
        return self.request("thread/resume", payload, response_model=ThreadResumeResponse)

    def thread_list(self, params: V2ThreadListParams | JsonObject | None = None) -> ThreadListResponse:
        return self.request("thread/list", _params_dict(params), response_model=ThreadListResponse)

    def thread_read(self, thread_id: str, include_turns: bool = False) -> ThreadReadResponse:
        return self.request(
            "thread/read",
            {"threadId": thread_id, "includeTurns": include_turns},
            response_model=ThreadReadResponse,
        )

    def thread_fork(
        self,
        thread_id: str,
        params: V2ThreadForkParams | JsonObject | None = None,
    ) -> ThreadForkResponse:
        payload = {"threadId": thread_id, **_params_dict(params)}
        return self.request("thread/fork", payload, response_model=ThreadForkResponse)

    def thread_archive(self, thread_id: str) -> ThreadArchiveResponse:
        return self.request("thread/archive", {"threadId": thread_id}, response_model=ThreadArchiveResponse)

    def thread_unarchive(self, thread_id: str) -> ThreadUnarchiveResponse:
        return self.request("thread/unarchive", {"threadId": thread_id}, response_model=ThreadUnarchiveResponse)

    def thread_set_name(self, thread_id: str, name: str) -> ThreadSetNameResponse:
        return self.request(
            "thread/setName",
            {"threadId": thread_id, "name": name},
            response_model=ThreadSetNameResponse,
        )

    def thread_compact(self, thread_id: str) -> ThreadCompactStartResponse:
        return self.request(
            "thread/compact",
            {"threadId": thread_id},
            response_model=ThreadCompactStartResponse,
        )

    def turn_start(
        self,
        thread_id: str,
        input_items: list[JsonObject] | JsonObject | str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TurnStartResponse:
        payload = {
            **_params_dict(params),
            "threadId": thread_id,
            "input": self._normalize_input_items(input_items),
        }
        return self.request("turn/start", payload, response_model=TurnStartResponse)

    def turn_text(
        self,
        thread_id: str,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TurnStartResponse:
        return self.turn_start(thread_id, text, params=params)

    def turn_interrupt(self, thread_id: str, turn_id: str) -> TurnInterruptResponse:
        return self.request(
            "turn/interrupt",
            {"threadId": thread_id, "turnId": turn_id},
            response_model=TurnInterruptResponse,
        )

    def turn_steer(
        self,
        thread_id: str,
        expected_turn_id: str,
        input_items: list[JsonObject] | JsonObject | str,
    ) -> TurnSteerResponse:
        return self.request(
            "turn/steer",
            {
                "threadId": thread_id,
                "expectedTurnId": expected_turn_id,
                "input": self._normalize_input_items(input_items),
            },
            response_model=TurnSteerResponse,
        )

    def model_list(self, include_hidden: bool = False) -> ModelListResponse:
        return self.request(
            "model/list",
            {"includeHidden": include_hidden},
            response_model=ModelListResponse,
        )

    def thread(self, thread_id: str) -> ThreadSession:
        return ThreadSession(client=self, thread_id=thread_id)

    def thread_start_session(
        self,
        *,
        model: str | None = None,
        params: V2ThreadStartParams | JsonObject | None = None,
    ) -> ThreadSession:
        payload = _params_dict(params)
        if model is not None:
            payload["model"] = model
        started = self.thread_start(payload)
        return ThreadSession(client=self, thread_id=started.thread.id)

    def request_with_retry_on_overload(
        self,
        method: str,
        params: JsonObject | None,
        *,
        response_model: type[ModelT],
        max_attempts: int = 3,
        initial_delay_s: float = 0.25,
        max_delay_s: float = 2.0,
    ) -> ModelT:
        return retry_on_overload(
            lambda: self.request(method, params, response_model=response_model),
            max_attempts=max_attempts,
            initial_delay_s=initial_delay_s,
            max_delay_s=max_delay_s,
        )

    def wait_for_turn_completed(self, turn_id: str) -> TurnCompletedNotification:
        while True:
            notification = self.next_notification()
            if (
                notification.method == "turn/completed"
                and isinstance(notification.payload, TurnCompletedNotification)
                and notification.payload.turn.id == turn_id
            ):
                return notification.payload

    def stream_until_methods(self, methods: Iterable[str] | str) -> list[Notification]:
        target_methods = {methods} if isinstance(methods, str) else set(methods)
        out: list[Notification] = []
        while True:
            notification = self.next_notification()
            out.append(notification)
            if notification.method in target_methods:
                return out

    def run_text_turn(
        self,
        thread_id: str,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> TextTurnResult:
        started = self.turn_text(thread_id, text, params=params)
        turn_id = started.turn.id

        deltas: list[AgentMessageDeltaNotification] = []
        completed: TurnCompletedNotification | None = None

        while True:
            notification = self.next_notification()
            if (
                notification.method == "item/agentMessage/delta"
                and isinstance(notification.payload, AgentMessageDeltaNotification)
                and notification.payload.turnId == turn_id
            ):
                deltas.append(notification.payload)
                continue
            if (
                notification.method == "turn/completed"
                and isinstance(notification.payload, TurnCompletedNotification)
                and notification.payload.turn.id == turn_id
            ):
                completed = notification.payload
                break

        if completed is None:
            raise AppServerError("turn/completed notification not received")

        return TextTurnResult(
            thread_id=thread_id,
            turn_id=turn_id,
            deltas=deltas,
            completed=completed,
        )

    def ask_result(
        self,
        text: str,
        *,
        model: str | None = None,
        thread_id: str | None = None,
    ) -> TextTurnResult:
        active_thread_id = thread_id
        if active_thread_id is None:
            start_params = V2ThreadStartParams(model=model) if model else None
            started = self.thread_start(start_params)
            active_thread_id = started.thread.id
        return self.run_text_turn(active_thread_id, text)

    def ask(
        self,
        text: str,
        *,
        model: str | None = None,
        thread_id: str | None = None,
    ) -> TextTurnResult:
        return self.ask_result(text, model=model, thread_id=thread_id)

    def stream_text(
        self,
        thread_id: str,
        text: str,
        params: V2TurnStartParams | JsonObject | None = None,
    ) -> Iterator[AgentMessageDeltaNotification]:
        started = self.turn_text(thread_id, text, params=params)
        turn_id = started.turn.id
        while True:
            notification = self.next_notification()
            if (
                notification.method == "item/agentMessage/delta"
                and isinstance(notification.payload, AgentMessageDeltaNotification)
                and notification.payload.turnId == turn_id
            ):
                yield notification.payload
                continue
            if (
                notification.method == "turn/completed"
                and isinstance(notification.payload, TurnCompletedNotification)
                and notification.payload.turn.id == turn_id
            ):
                break

    def _coerce_notification(self, method: str, params: object) -> Notification:
        model = _NOTIFICATION_MODELS.get(method)
        if model is None:
            raise AppServerError(f"Unsupported notification method: {method}")
        params_dict = params if isinstance(params, dict) else {}
        payload = model.model_validate(params_dict)
        return Notification(method=method, payload=payload)

    def _normalize_input_items(
        self, input_items: list[JsonObject] | JsonObject | str
    ) -> list[JsonObject]:
        if isinstance(input_items, str):
            return [{"type": "text", "text": input_items}]
        if isinstance(input_items, dict):
            return [input_items]
        return input_items

    def _default_approval_handler(self, method: str, params: JsonObject | None) -> JsonObject:
        if method == "item/commandExecution/requestApproval":
            return {"decision": "accept"}
        if method == "item/fileChange/requestApproval":
            return {"decision": "accept"}
        return {}

    def _start_stderr_drain_thread(self) -> None:
        if self._proc is None or self._proc.stderr is None:
            return

        def _drain() -> None:
            stderr = self._proc.stderr
            if stderr is None:
                return
            for line in stderr:
                self._stderr_lines.append(line.rstrip("\n"))

        self._stderr_thread = threading.Thread(target=_drain, daemon=True)
        self._stderr_thread.start()

    def _stderr_tail(self, limit: int = 40) -> str:
        return "\n".join(list(self._stderr_lines)[-limit:])

    def _handle_server_request(self, msg: JsonObject) -> JsonObject:
        method = msg["method"]
        params = msg.get("params")
        request_params = params if isinstance(params, dict) else None
        return self._approval_handler(str(method), request_params)

    def _write_message(self, payload: JsonObject) -> None:
        if self._proc is None or self._proc.stdin is None:
            raise TransportClosedError("app-server is not running")
        with self._lock:
            self._proc.stdin.write(json.dumps(payload) + "\n")
            self._proc.stdin.flush()

    def _read_message(self) -> JsonObject:
        if self._proc is None or self._proc.stdout is None:
            raise TransportClosedError("app-server is not running")

        line = self._proc.stdout.readline()
        if not line:
            raise TransportClosedError(
                f"app-server closed stdout. stderr_tail={self._stderr_tail()[:2000]}"
            )

        try:
            payload = json.loads(line)
        except json.JSONDecodeError as exc:
            raise AppServerError(f"Invalid JSON-RPC line: {line!r}") from exc
        if not isinstance(payload, dict):
            raise AppServerError(f"Expected JSON object message, got: {type(payload).__name__}")
        return payload


def default_codex_home() -> Path:
    return Path.home() / ".codex"
