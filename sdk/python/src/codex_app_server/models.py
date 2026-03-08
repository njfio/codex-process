from __future__ import annotations

from dataclasses import dataclass
from typing import TypeAlias

from pydantic import BaseModel

from .generated.codex_event_types import CodexEventNotification
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
from .generated.v2_all.ThreadNameUpdatedNotification import ThreadNameUpdatedNotification
from .generated.v2_all.ThreadStartedNotification import ThreadStartedNotification
from .generated.v2_all.ThreadTokenUsageUpdatedNotification import (
    ThreadTokenUsageUpdatedNotification,
)
from .generated.v2_all.TurnCompletedNotification import TurnCompletedNotification
from .generated.v2_all.TurnDiffUpdatedNotification import TurnDiffUpdatedNotification
from .generated.v2_all.TurnPlanUpdatedNotification import TurnPlanUpdatedNotification
from .generated.v2_all.TurnStartedNotification import TurnStartedNotification
from .generated.v2_all.WindowsWorldWritableWarningNotification import (
    WindowsWorldWritableWarningNotification,
)

JsonScalar: TypeAlias = str | int | float | bool | None
JsonValue: TypeAlias = JsonScalar | dict[str, "JsonValue"] | list["JsonValue"]
JsonObject: TypeAlias = dict[str, JsonValue]


@dataclass(slots=True)
class UnknownNotification:
    params: JsonObject


NotificationPayload: TypeAlias = (
    AccountLoginCompletedNotification
    | AccountRateLimitsUpdatedNotification
    | AccountUpdatedNotification
    | AgentMessageDeltaNotification
    | AppListUpdatedNotification
    | CommandExecutionOutputDeltaNotification
    | ConfigWarningNotification
    | ContextCompactedNotification
    | DeprecationNoticeNotification
    | ErrorNotification
    | FileChangeOutputDeltaNotification
    | ItemCompletedNotification
    | ItemStartedNotification
    | McpServerOauthLoginCompletedNotification
    | McpToolCallProgressNotification
    | PlanDeltaNotification
    | RawResponseItemCompletedNotification
    | ReasoningSummaryPartAddedNotification
    | ReasoningSummaryTextDeltaNotification
    | ReasoningTextDeltaNotification
    | TerminalInteractionNotification
    | ThreadNameUpdatedNotification
    | ThreadStartedNotification
    | ThreadTokenUsageUpdatedNotification
    | TurnCompletedNotification
    | TurnDiffUpdatedNotification
    | TurnPlanUpdatedNotification
    | TurnStartedNotification
    | WindowsWorldWritableWarningNotification
    | CodexEventNotification
    | UnknownNotification
)


@dataclass(slots=True)
class Notification:
    method: str
    payload: NotificationPayload


class ServerInfo(BaseModel):
    name: str | None = None
    version: str | None = None


class InitializeResponse(BaseModel):
    serverInfo: ServerInfo | None = None
    userAgent: str | None = None


@dataclass(slots=True)
class TextTurnResult:
    thread_id: str
    turn_id: str
    deltas: list[AgentMessageDeltaNotification]
    completed: TurnCompletedNotification
