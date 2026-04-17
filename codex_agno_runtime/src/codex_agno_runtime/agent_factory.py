"""Agent factory for the Codex Agno runtime."""

from __future__ import annotations

from typing import Any

from codex_agno_runtime.execution_kernel_contracts import (
    build_execution_kernel_compatibility_agent_instructions,
)


class _UnavailableAgent:
    """Fallback agent returned when Agno is not installed."""

    def __init__(self, instructions: list[str] | None = None) -> None:
        self.instructions = instructions or []

    async def arun(self, *args: Any, **kwargs: Any) -> Any:  # pragma: no cover - defensive live-path guard
        raise RuntimeError("Agno is not installed; live runtime execution is unavailable.")


class AgentFactory:
    """Build compatibility agents for the Python fallback path."""

    def __init__(self, settings: Any, prompt_builder: Any) -> None:
        self.settings = settings
        self.prompt_builder = prompt_builder

    def build_compatibility_agent(self, routing_result: Any, user_id: str) -> Any:
        """Build a fallback agent instance.

        Parameters:
            routing_result: Resolved routing result.
            user_id: Active user id.

        Returns:
            Any: Agent-like object with an async `arun()` method.
        """

        instructions = build_execution_kernel_compatibility_agent_instructions(
            routing_result=routing_result,
            build_prompt=self.prompt_builder.build_prompt,
        )

        try:
            from agno.agent import Agent  # type: ignore
        except Exception:
            return _UnavailableAgent(instructions=instructions)

        try:
            return Agent(
                instructions=instructions,
                markdown=True,
                add_datetime_to_instructions=False,
            )
        except Exception:
            return _UnavailableAgent(instructions=instructions)

    def build_agent(self, routing_result: Any, user_id: str) -> Any:
        """Backward-compatible alias for the Python fallback path."""

        return self.build_compatibility_agent(routing_result, user_id)
