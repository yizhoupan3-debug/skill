"""Agent factory for the Codex Agno runtime."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any

from codex_agno_runtime.execution_kernel_contracts import (
    ExecutionKernelCompatibilityAgentSpec,
    build_execution_kernel_compatibility_agent_spec,
)


class _UnavailableAgent:
    """Fallback agent returned when Agno is not installed."""

    def __init__(self, instructions: list[str] | None = None) -> None:
        self.instructions = instructions or []

    async def arun(self, *args: Any, **kwargs: Any) -> Any:  # pragma: no cover - defensive live-path guard
        raise RuntimeError("Agno is not installed; live runtime execution is unavailable.")


@dataclass(slots=True)
class CompatibilityAgentHandle:
    """Compatibility-only Python agent plus the contract used to construct it."""

    agent: Any
    contract: ExecutionKernelCompatibilityAgentSpec


class AgentFactory:
    """Build compatibility agents for the Python fallback path."""

    def __init__(self, settings: Any, prompt_builder: Any) -> None:
        self.settings = settings
        self.prompt_builder = prompt_builder

    def build_compatibility_agent_handle(
        self,
        routing_result: Any,
        user_id: str,
        *,
        prompt_preview: str | None = None,
    ) -> CompatibilityAgentHandle:
        """Build a compatibility agent handle for the Python fallback lane.

        Parameters:
            routing_result: Resolved routing result.
            user_id: Active user id.
            prompt_preview: Explicit prompt preview owned by the caller when available.

        Returns:
            CompatibilityAgentHandle: Agent plus the construction contract metadata.
        """

        contract = build_execution_kernel_compatibility_agent_spec(
            routing_result=routing_result,
            build_prompt=self.prompt_builder.build_prompt,
            prompt_preview=prompt_preview,
        )
        instructions = [*contract.instructions]

        try:
            from agno.agent import Agent  # type: ignore
        except Exception:
            return CompatibilityAgentHandle(
                agent=_UnavailableAgent(instructions=instructions),
                contract=contract,
            )

        try:
            agent = Agent(
                instructions=instructions,
                markdown=True,
                add_datetime_to_instructions=False,
            )
            return CompatibilityAgentHandle(agent=agent, contract=contract)
        except Exception:
            return CompatibilityAgentHandle(
                agent=_UnavailableAgent(instructions=instructions),
                contract=contract,
            )

    def build_compatibility_agent(
        self,
        routing_result: Any,
        user_id: str,
        *,
        prompt_preview: str | None = None,
    ) -> Any:
        """Build a compatibility agent instance without exposing the contract handle."""

        return self.build_compatibility_agent_handle(
            routing_result,
            user_id,
            prompt_preview=prompt_preview,
        ).agent

    def build_agent(self, routing_result: Any, user_id: str) -> Any:
        """Backward-compatible alias for the Python fallback path."""

        return self.build_compatibility_agent(routing_result, user_id)
