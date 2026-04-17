"""Shared data models for the browser MCP skeleton."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any


JSONDict = dict[str, Any]


@dataclass(slots=True)
class PageElement:
    """Store a browser element reference and its agent-facing metadata.

    Parameters:
        ref: Stable element reference exposed to the agent.
        role: Accessibility or semantic role.
        name: Primary human-readable name.
        text: Optional visible text payload.
        visible: Whether the element is currently visible.
        enabled: Whether the element is interactive.
        value: Current value for form-like controls.
        locator_hint: Optional low-entropy locator hints for debugging.
        action_id: Internal action hook for simulated transitions.

    Returns:
        PageElement: Element model instance.
    """

    ref: str
    role: str
    name: str
    text: str = ""
    visible: bool = True
    enabled: bool = True
    value: str = ""
    locator_hint: JSONDict | None = None
    action_id: str | None = None

    def to_public_dict(self) -> JSONDict:
        """Serialize the element for tool responses.

        Parameters:
            None.

        Returns:
            dict[str, Any]: Public element fields for agents.
        """

        payload: JSONDict = {
            "ref": self.ref,
            "role": self.role,
            "name": self.name,
            "text": self.text,
            "visible": self.visible,
            "enabled": self.enabled,
        }
        if self.value:
            payload["value"] = self.value
        if self.locator_hint:
            payload["locator_hint"] = self.locator_hint
        return payload


@dataclass(slots=True)
class BrowserSession:
    """Represent the top-level browser session state.

    Parameters:
        session_id: Stable browser session identifier.
        current_tab_id: Currently selected tab identifier.
        viewport: Viewport dimensions.

    Returns:
        BrowserSession: Session model instance.
    """

    session_id: str
    current_tab_id: str | None
    viewport: JSONDict = field(default_factory=lambda: {"width": 1440, "height": 900})

    def to_dict(self) -> JSONDict:
        """Serialize the session into a JSON-friendly payload.

        Parameters:
            None.

        Returns:
            dict[str, Any]: Session payload.
        """

        return {
            "session_id": self.session_id,
            "current_tab_id": self.current_tab_id,
            "viewport": self.viewport,
        }


@dataclass(slots=True)
class BrowserTab:
    """Represent an open browser tab.

    Parameters:
        tab_id: Stable tab identifier.
        url: Current tab URL.
        title: Current tab title.
        page_revision: Monotonic page revision counter.
        loading_state: High-level loading status.

    Returns:
        BrowserTab: Tab model instance.
    """

    tab_id: str
    url: str
    title: str
    page_revision: int
    loading_state: str = "idle"

    def to_dict(self) -> JSONDict:
        """Serialize the tab into a JSON-friendly payload.

        Parameters:
            None.

        Returns:
            dict[str, Any]: Tab payload.
        """

        return {
            "tab_id": self.tab_id,
            "url": self.url,
            "title": self.title,
            "page_revision": self.page_revision,
            "loading_state": self.loading_state,
        }


@dataclass(slots=True)
class PageDelta:
    """Capture the user-visible changes between two revisions.

    Parameters:
        from_revision: Previous page revision.
        to_revision: New page revision.
        url_changed: Whether navigation changed the URL.
        title_changed: Whether the page title changed.
        new_elements: Newly exposed interactive elements.
        removed_refs: Removed element references.
        new_text: Newly visible text snippets.
        alerts: User-visible alert or error messages.

    Returns:
        PageDelta: Delta model instance.
    """

    from_revision: int
    to_revision: int
    url_changed: bool = False
    title_changed: bool = False
    new_elements: list[PageElement] = field(default_factory=list)
    removed_refs: list[str] = field(default_factory=list)
    new_text: list[str] = field(default_factory=list)
    alerts: list[str] = field(default_factory=list)

    def merge(self, other: "PageDelta") -> "PageDelta":
        """Merge another delta into this delta for aggregated reporting.

        Parameters:
            other: A later delta in the same tab history.

        Returns:
            PageDelta: Combined delta spanning both revisions.
        """

        return PageDelta(
            from_revision=self.from_revision,
            to_revision=other.to_revision,
            url_changed=self.url_changed or other.url_changed,
            title_changed=self.title_changed or other.title_changed,
            new_elements=[*self.new_elements, *other.new_elements],
            removed_refs=[*self.removed_refs, *other.removed_refs],
            new_text=[*self.new_text, *other.new_text],
            alerts=[*self.alerts, *other.alerts],
        )

    def to_dict(self) -> JSONDict:
        """Serialize the delta into a JSON-friendly payload.

        Parameters:
            None.

        Returns:
            dict[str, Any]: Delta payload.
        """

        return {
            "from_revision": self.from_revision,
            "to_revision": self.to_revision,
            "url_changed": self.url_changed,
            "title_changed": self.title_changed,
            "new_elements": [element.to_public_dict() for element in self.new_elements],
            "removed_refs": self.removed_refs,
            "new_text": self.new_text,
            "alerts": self.alerts,
        }


@dataclass(slots=True)
class PageSummary:
    """Summarize the current page using a token-bounded shape.

    Parameters:
        main_goal_area: High-signal summary of the active area.
        visible_messages: User-visible messages in the active area.
        forms: Count of forms on the page.
        dialogs: Count of visible dialogs.

    Returns:
        PageSummary: Summary model instance.
    """

    main_goal_area: str
    visible_messages: list[str]
    forms: int
    dialogs: int

    def to_dict(self) -> JSONDict:
        """Serialize the summary into a JSON-friendly payload.

        Parameters:
            None.

        Returns:
            dict[str, Any]: Summary payload.
        """

        return {
            "main_goal_area": self.main_goal_area,
            "visible_messages": self.visible_messages,
            "forms": self.forms,
            "dialogs": self.dialogs,
        }


@dataclass(slots=True)
class WaitCondition:
    """Represent a normalized wait condition.

    Parameters:
        type: Supported condition type.
        value: Condition-specific comparison value.

    Returns:
        WaitCondition: Wait condition model instance.
    """

    type: str
    value: str

    @classmethod
    def from_dict(cls, raw: JSONDict) -> "WaitCondition":
        """Build a wait condition from raw input arguments.

        Parameters:
            raw: Untrusted input dictionary from a tool call.

        Returns:
            WaitCondition: Normalized wait condition.
        """

        return cls(type=str(raw["type"]), value=str(raw["value"]))
