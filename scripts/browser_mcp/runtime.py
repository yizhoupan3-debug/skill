"""Runtime abstractions and an in-memory backend for the browser MCP skeleton."""

from __future__ import annotations

import time
from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any
from urllib.parse import urlparse

from .errors import BrowserServerError
from .models import BrowserSession, BrowserTab, PageDelta, PageElement, PageSummary, WaitCondition


JSONDict = dict[str, Any]


@dataclass(slots=True)
class _TabState:
    """Store mutable in-memory tab state for the demo backend.

    Parameters:
        tab: Current tab metadata.
        goal_area: High-level description of the main UI area.
        messages: Current visible messages.
        dialogs: Current dialog count.
        forms: Current form count.
        elements: Current interactive elements keyed by reference.
        history: Revision deltas emitted so far.

    Returns:
        _TabState: Internal tab state instance.
    """

    tab: BrowserTab
    goal_area: str
    messages: list[str]
    dialogs: int
    forms: int
    elements: dict[str, PageElement] = field(default_factory=dict)
    history: list[PageDelta] = field(default_factory=list)


class BrowserRuntime(ABC):
    """Define the operations required by the browser MCP server.

    Parameters:
        None.

    Returns:
        BrowserRuntime: Abstract runtime interface.
    """

    @abstractmethod
    def open_page(self, url: str, new_tab: bool) -> JSONDict:
        """Open a page and return session plus current tab details.

        Parameters:
            url: Target URL to open.
            new_tab: Whether to create a new tab or reuse the current one.

        Returns:
            dict[str, Any]: Session and tab payload.
        """

    @abstractmethod
    def tabs(self, action: str, tab_id: str | None = None) -> JSONDict:
        """List or select tabs in the current session.

        Parameters:
            action: Supported tab action such as list or select.
            tab_id: Optional tab identifier for selection.

        Returns:
            dict[str, Any]: Tab state payload.
        """

    @abstractmethod
    def close(self, target: str, tab_id: str | None = None) -> JSONDict:
        """Close a tab or the entire session.

        Parameters:
            target: Close target type, such as tab or session.
            tab_id: Optional tab identifier when closing a specific tab.

        Returns:
            dict[str, Any]: Remaining session state.
        """

    @abstractmethod
    def get_state(
        self,
        tab_id: str | None,
        include: list[str],
        since_revision: int | None,
        max_elements: int,
        text_budget: int,
    ) -> JSONDict:
        """Return a compressed snapshot of the current page.

        Parameters:
            tab_id: Optional target tab identifier.
            include: Requested state sections.
            since_revision: Optional revision baseline for delta reporting.
            max_elements: Maximum interactive elements to include.
            text_budget: Maximum character budget for summary text.

        Returns:
            dict[str, Any]: Compressed page state.
        """

    @abstractmethod
    def get_elements(
        self,
        tab_id: str | None,
        role: str | None,
        query: str | None,
        scope_ref: str | None,
        limit: int,
    ) -> JSONDict:
        """Find interactive elements matching the provided constraints.

        Parameters:
            tab_id: Optional target tab identifier.
            role: Optional element role filter.
            query: Optional textual search query.
            scope_ref: Optional scope element reference.
            limit: Maximum number of matches to return.

        Returns:
            dict[str, Any]: Matching elements payload.
        """

    @abstractmethod
    def click(self, tab_id: str | None, ref: str, timeout_ms: int) -> JSONDict:
        """Click an element and return the resulting delta.

        Parameters:
            tab_id: Optional target tab identifier.
            ref: Element reference to click.
            timeout_ms: Client-provided timeout budget.

        Returns:
            dict[str, Any]: Action result and resulting page delta.
        """

    @abstractmethod
    def fill(self, tab_id: str | None, ref: str, value: str, submit: bool) -> JSONDict:
        """Fill an input-like element and optionally submit it.

        Parameters:
            tab_id: Optional target tab identifier.
            ref: Element reference to fill.
            value: New value to assign.
            submit: Whether to submit after filling.

        Returns:
            dict[str, Any]: Action result and resulting page delta.
        """

    @abstractmethod
    def wait_for(self, tab_id: str | None, condition: WaitCondition, timeout_ms: int) -> JSONDict:
        """Wait until a supported condition becomes true.

        Parameters:
            tab_id: Optional target tab identifier.
            condition: Normalized wait condition.
            timeout_ms: Maximum wait time in milliseconds.

        Returns:
            dict[str, Any]: Wait result payload.
        """


class InMemoryBrowserRuntime(BrowserRuntime):
    """Provide a deterministic browser runtime for local development and tests.

    Parameters:
        None.

    Returns:
        InMemoryBrowserRuntime: Ready-to-use demo runtime.
    """

    def __init__(self) -> None:
        """Initialize the in-memory session and tab stores.

        Parameters:
            None.

        Returns:
            None.
        """

        self._session = BrowserSession(session_id="sess_001", current_tab_id=None)
        self._tabs: dict[str, _TabState] = {}
        self._tab_counter = 0

    def open_page(self, url: str, new_tab: bool) -> JSONDict:
        """Open a URL in the current tab or a new tab.

        Parameters:
            url: Target URL to open.
            new_tab: Whether to open a new tab.

        Returns:
            dict[str, Any]: Session and tab payload.
        """

        tab_id = self._create_tab_id() if new_tab or self._session.current_tab_id is None else self._session.current_tab_id
        state = self._build_state(tab_id=tab_id, url=url)
        self._tabs[tab_id] = state
        self._session.current_tab_id = tab_id
        return {"session": self._session.to_dict(), "tab": state.tab.to_dict()}

    def tabs(self, action: str, tab_id: str | None = None) -> JSONDict:
        """List tabs or switch the active tab.

        Parameters:
            action: list or select.
            tab_id: Optional tab identifier when selecting.

        Returns:
            dict[str, Any]: Tab listing or selected tab payload.
        """

        if action == "list":
            return {
                "session": self._session.to_dict(),
                "tabs": [state.tab.to_dict() for state in self._tabs.values()],
            }
        if action == "select":
            state = self._require_tab(tab_id)
            self._session.current_tab_id = state.tab.tab_id
            return {"session": self._session.to_dict(), "tab": state.tab.to_dict()}
        raise BrowserServerError(
            code="INVALID_INPUT",
            message=f"Unsupported browser_tabs action: {action}",
            suggested_next_actions=["call browser_tabs with action='list' or action='select'"],
        )

    def close(self, target: str, tab_id: str | None = None) -> JSONDict:
        """Close a tab or the entire in-memory session.

        Parameters:
            target: tab or session.
            tab_id: Optional tab identifier when closing a single tab.

        Returns:
            dict[str, Any]: Remaining session state.
        """

        if target == "session":
            self._tabs.clear()
            self._session.current_tab_id = None
            return {"session": self._session.to_dict(), "closed": "session"}
        if target == "tab":
            state = self._require_tab(tab_id)
            del self._tabs[state.tab.tab_id]
            if self._session.current_tab_id == state.tab.tab_id:
                self._session.current_tab_id = next(iter(self._tabs), None)
            return {"session": self._session.to_dict(), "closed": state.tab.tab_id}
        raise BrowserServerError(
            code="INVALID_INPUT",
            message=f"Unsupported browser_close target: {target}",
            suggested_next_actions=["call browser_close with target='tab' or target='session'"],
        )

    def get_state(
        self,
        tab_id: str | None,
        include: list[str],
        since_revision: int | None,
        max_elements: int,
        text_budget: int,
    ) -> JSONDict:
        """Return a compressed page state payload.

        Parameters:
            tab_id: Optional target tab identifier.
            include: Requested sections.
            since_revision: Optional delta baseline.
            max_elements: Maximum interactive elements to return.
            text_budget: Maximum character budget for summary content.

        Returns:
            dict[str, Any]: Compressed state payload.
        """

        state = self._require_tab(tab_id)
        payload: JSONDict = {"tab": state.tab.to_dict()}
        if since_revision is not None and since_revision == state.tab.page_revision:
            payload["unchanged"] = True
            payload["page_revision"] = state.tab.page_revision
            return payload
        if "summary" in include:
            payload["summary"] = self._build_summary(state, text_budget).to_dict()
        if "interactive_elements" in include:
            payload["interactive_elements"] = [
                element.to_public_dict()
                for element in list(state.elements.values())[:max_elements]
            ]
        if "diff" in include and since_revision is not None:
            payload["diff"] = self._aggregate_delta(state, since_revision).to_dict()
        return payload

    def get_elements(
        self,
        tab_id: str | None,
        role: str | None,
        query: str | None,
        scope_ref: str | None,
        limit: int,
    ) -> JSONDict:
        """Return interactive elements filtered by role and text query.

        Parameters:
            tab_id: Optional target tab identifier.
            role: Optional role filter.
            query: Optional text filter.
            scope_ref: Optional scope reference.
            limit: Maximum number of matches.

        Returns:
            dict[str, Any]: Match list payload.
        """

        state = self._require_tab(tab_id)
        scope_prefix = f"{scope_ref}." if scope_ref else ""
        query_text = (query or "").lower()
        matches: list[JSONDict] = []
        for element in state.elements.values():
            haystack = f"{element.name} {element.text}".lower()
            if role and element.role != role:
                continue
            if query_text and query_text not in haystack:
                continue
            if scope_prefix and not element.ref.startswith(scope_prefix):
                continue
            matches.append(element.to_public_dict())
            if len(matches) >= limit:
                break
        return {"matches": matches}

    def click(self, tab_id: str | None, ref: str, timeout_ms: int) -> JSONDict:
        """Click an element and apply a deterministic state transition.

        Parameters:
            tab_id: Optional target tab identifier.
            ref: Element reference to click.
            timeout_ms: Client timeout budget, retained for parity.

        Returns:
            dict[str, Any]: Action result payload.
        """

        state = self._require_tab(tab_id)
        element = self._require_element(state, ref)
        self._ensure_interactable(element)
        _ = timeout_ms
        if element.action_id == "submit_login":
            previous_tab = state.tab
            new_state = self._build_state(tab_id=state.tab.tab_id, url="https://example.com/dashboard", previous_revision=state.tab.page_revision)
            delta = PageDelta(
                from_revision=previous_tab.page_revision,
                to_revision=new_state.tab.page_revision,
                url_changed=True,
                title_changed=True,
                new_text=["Recent activity"],
                removed_refs=list(state.elements.keys()),
                new_elements=list(new_state.elements.values()),
            )
            new_state.history = [*state.history, delta]
            self._tabs[state.tab.tab_id] = new_state
            return {
                "ok": True,
                "action": "click",
                "ref": ref,
                "tab": new_state.tab.to_dict(),
                "delta": delta.to_dict(),
            }
        delta = self._bump_revision(
            state=state,
            new_text=[f"Clicked {element.name}"],
            removed_refs=[],
            new_elements=[],
        )
        return {
            "ok": True,
            "action": "click",
            "ref": ref,
            "tab": state.tab.to_dict(),
            "delta": delta.to_dict(),
        }

    def fill(self, tab_id: str | None, ref: str, value: str, submit: bool) -> JSONDict:
        """Fill an element value and optionally submit the active form.

        Parameters:
            tab_id: Optional target tab identifier.
            ref: Element reference to fill.
            value: New value to assign.
            submit: Whether to immediately submit after filling.

        Returns:
            dict[str, Any]: Action result payload.
        """

        state = self._require_tab(tab_id)
        element = self._require_element(state, ref)
        self._ensure_interactable(element)
        element.value = value
        delta = self._bump_revision(
            state=state,
            new_text=[f"Updated {element.name}"],
            removed_refs=[],
            new_elements=[],
        )
        result: JSONDict = {
            "ok": True,
            "action": "fill",
            "ref": ref,
            "tab": state.tab.to_dict(),
            "delta": delta.to_dict(),
        }
        if submit:
            result["submitted"] = True
        return result

    def wait_for(self, tab_id: str | None, condition: WaitCondition, timeout_ms: int) -> JSONDict:
        """Wait until the condition is satisfied or raise a timeout error.

        Parameters:
            tab_id: Optional target tab identifier.
            condition: Condition to evaluate.
            timeout_ms: Maximum wait time in milliseconds.

        Returns:
            dict[str, Any]: Wait result payload.
        """

        state = self._require_tab(tab_id)
        deadline = time.time() + max(timeout_ms, 0) / 1000
        while time.time() <= deadline:
            if self._condition_matches(state, condition):
                return {
                    "ok": True,
                    "condition": {"type": condition.type, "value": condition.value},
                    "tab": state.tab.to_dict(),
                }
            time.sleep(0.01)
        raise BrowserServerError(
            code="TIMEOUT",
            message=f"Condition {condition.type}='{condition.value}' not met within {timeout_ms}ms.",
            suggested_next_actions=["call browser_get_state", "call browser_get_elements"],
        )

    def _create_tab_id(self) -> str:
        """Generate the next stable tab identifier.

        Parameters:
            None.

        Returns:
            str: New tab identifier.
        """

        self._tab_counter += 1
        return f"tab_{self._tab_counter:02d}"

    def _require_tab(self, tab_id: str | None) -> _TabState:
        """Resolve the requested or current tab state.

        Parameters:
            tab_id: Optional tab identifier.

        Returns:
            _TabState: Matching tab state.
        """

        resolved_tab_id = tab_id or self._session.current_tab_id
        if not resolved_tab_id or resolved_tab_id not in self._tabs:
            raise BrowserServerError(
                code="TAB_NOT_FOUND",
                message=f"Browser tab not found: {resolved_tab_id}",
                suggested_next_actions=["call browser_open", "call browser_tabs with action='list'"],
            )
        return self._tabs[resolved_tab_id]

    def _require_element(self, state: _TabState, ref: str) -> PageElement:
        """Resolve an element reference or emit a stale reference error.

        Parameters:
            state: Target tab state.
            ref: Element reference to resolve.

        Returns:
            PageElement: Matching element model.
        """

        element = state.elements.get(ref)
        if element is None:
            raise BrowserServerError(
                code="STALE_ELEMENT_REF",
                message=(
                    f"Element ref {ref} is not available in tab {state.tab.tab_id} "
                    f"revision {state.tab.page_revision}."
                ),
                suggested_next_actions=[
                    "call browser_get_state",
                    "call browser_get_elements with a narrowed query",
                ],
            )
        return element

    def _ensure_interactable(self, element: PageElement) -> None:
        """Validate that an element can be interacted with.

        Parameters:
            element: Element to validate.

        Returns:
            None.
        """

        if not element.visible:
            raise BrowserServerError(
                code="ELEMENT_NOT_VISIBLE",
                message=f"Element {element.ref} is not visible.",
                suggested_next_actions=["call browser_get_state", "call browser_get_elements"],
            )
        if not element.enabled:
            raise BrowserServerError(
                code="ELEMENT_NOT_ENABLED",
                message=f"Element {element.ref} is disabled.",
                suggested_next_actions=["call browser_get_state", "call browser_get_elements"],
            )

    def _build_summary(self, state: _TabState, text_budget: int) -> PageSummary:
        """Build a token-bounded page summary.

        Parameters:
            state: Target tab state.
            text_budget: Character budget for visible messages.

        Returns:
            PageSummary: Compressed page summary.
        """

        remaining = max(text_budget, 0)
        messages: list[str] = []
        for message in state.messages:
            clipped = message[:remaining]
            if not clipped:
                break
            messages.append(clipped)
            remaining -= len(clipped)
        return PageSummary(
            main_goal_area=state.goal_area[:text_budget],
            visible_messages=messages,
            forms=state.forms,
            dialogs=state.dialogs,
        )

    def _aggregate_delta(self, state: _TabState, since_revision: int) -> PageDelta:
        """Aggregate deltas newer than the provided revision.

        Parameters:
            state: Target tab state.
            since_revision: Baseline revision.

        Returns:
            PageDelta: Aggregated delta across newer revisions.
        """

        deltas = [delta for delta in state.history if delta.to_revision > since_revision]
        if not deltas:
            return PageDelta(from_revision=since_revision, to_revision=state.tab.page_revision)
        merged = deltas[0]
        for delta in deltas[1:]:
            merged = merged.merge(delta)
        return merged

    def _bump_revision(
        self,
        state: _TabState,
        new_text: list[str],
        removed_refs: list[str],
        new_elements: list[PageElement],
    ) -> PageDelta:
        """Increment the current tab revision and record a delta.

        Parameters:
            state: Target tab state.
            new_text: Newly visible text snippets.
            removed_refs: Removed element references.
            new_elements: Newly exposed interactive elements.

        Returns:
            PageDelta: Newly recorded delta.
        """

        previous_revision = state.tab.page_revision
        state.tab.page_revision += 1
        delta = PageDelta(
            from_revision=previous_revision,
            to_revision=state.tab.page_revision,
            new_text=new_text,
            removed_refs=removed_refs,
            new_elements=new_elements,
        )
        state.history.append(delta)
        return delta

    def _condition_matches(self, state: _TabState, condition: WaitCondition) -> bool:
        """Evaluate a wait condition against the current tab state.

        Parameters:
            state: Target tab state.
            condition: Normalized wait condition.

        Returns:
            bool: True when the condition is satisfied.
        """

        if condition.type == "url_contains":
            return condition.value in state.tab.url
        if condition.type == "text_appears":
            haystack = " ".join([state.goal_area, *state.messages, *[element.text for element in state.elements.values()]])
            return condition.value in haystack
        if condition.type == "element_appears":
            return condition.value in state.elements
        if condition.type == "network_idle":
            return True
        raise BrowserServerError(
            code="UNSUPPORTED_OPERATION",
            message=f"Unsupported wait condition type: {condition.type}",
            suggested_next_actions=["call browser_wait_for with url_contains, text_appears, element_appears, or network_idle"],
        )

    def _build_state(self, tab_id: str, url: str, previous_revision: int = 0) -> _TabState:
        """Create an in-memory page state for the requested URL.

        Parameters:
            tab_id: Tab identifier being populated.
            url: URL to represent.
            previous_revision: Previous revision before navigation.

        Returns:
            _TabState: Initialized tab state.
        """

        parsed = urlparse(url)
        normalized_path = parsed.path or "/"
        if "login" in normalized_path:
            return self._build_login_state(tab_id=tab_id, url=url, previous_revision=previous_revision)
        if "dashboard" in normalized_path:
            return self._build_dashboard_state(tab_id=tab_id, url=url, previous_revision=previous_revision)
        return self._build_generic_state(tab_id=tab_id, url=url, previous_revision=previous_revision)

    def _build_login_state(self, tab_id: str, url: str, previous_revision: int) -> _TabState:
        """Build the deterministic login scenario state.

        Parameters:
            tab_id: Target tab identifier.
            url: Login page URL.
            previous_revision: Previous tab revision before navigation.

        Returns:
            _TabState: Login page state.
        """

        revision = previous_revision + 1
        elements = {
            "el_email": PageElement(
                ref="el_email",
                role="textbox",
                name="Email",
                locator_hint={"tag": "input", "name": "email"},
            ),
            "el_password": PageElement(
                ref="el_password",
                role="textbox",
                name="Password",
                locator_hint={"tag": "input", "name": "password"},
            ),
            "el_signin": PageElement(
                ref="el_signin",
                role="button",
                name="Sign in",
                text="Sign in",
                locator_hint={"tag": "button", "test_id": "sign-in"},
                action_id="submit_login",
            ),
        }
        return _TabState(
            tab=BrowserTab(tab_id=tab_id, url=url, title="Login - Example", page_revision=revision),
            goal_area="Login form with email and password fields",
            messages=["Welcome back"],
            dialogs=0,
            forms=1,
            elements=elements,
        )

    def _build_dashboard_state(self, tab_id: str, url: str, previous_revision: int) -> _TabState:
        """Build the deterministic dashboard scenario state.

        Parameters:
            tab_id: Target tab identifier.
            url: Dashboard page URL.
            previous_revision: Previous tab revision before navigation.

        Returns:
            _TabState: Dashboard page state.
        """

        revision = previous_revision + 1
        elements = {
            "el_billing": PageElement(
                ref="el_billing",
                role="link",
                name="Billing",
                text="Billing",
                locator_hint={"tag": "a", "href": "/billing"},
            ),
            "el_search": PageElement(
                ref="el_search",
                role="textbox",
                name="Search",
                locator_hint={"tag": "input", "name": "search"},
            ),
        }
        return _TabState(
            tab=BrowserTab(tab_id=tab_id, url=url, title="Dashboard", page_revision=revision),
            goal_area="Dashboard overview with recent activity and navigation",
            messages=["Recent activity"],
            dialogs=0,
            forms=1,
            elements=elements,
        )

    def _build_generic_state(self, tab_id: str, url: str, previous_revision: int) -> _TabState:
        """Build a generic fallback state for arbitrary URLs.

        Parameters:
            tab_id: Target tab identifier.
            url: Arbitrary page URL.
            previous_revision: Previous tab revision before navigation.

        Returns:
            _TabState: Generic page state.
        """

        revision = previous_revision + 1
        title_host = urlparse(url).netloc or "Example"
        elements = {
            "el_continue": PageElement(
                ref="el_continue",
                role="button",
                name="Continue",
                text="Continue",
                locator_hint={"tag": "button"},
            )
        }
        return _TabState(
            tab=BrowserTab(tab_id=tab_id, url=url, title=f"{title_host} Page", page_revision=revision),
            goal_area=f"Generic page for {title_host}",
            messages=[f"Loaded {url}"],
            dialogs=0,
            forms=0,
            elements=elements,
        )
