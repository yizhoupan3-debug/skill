"""Pluggable middleware chain for the Codex Agno runtime.

Inspired by DeerFlow 2.0's 11-layer middleware architecture.
Each middleware can hook into before_agent and after_agent phases,
enabling cross-cutting concerns like context compression, memory
injection, and sub-agent limits without modifying the core loop.
"""

from __future__ import annotations

import asyncio
import logging
from abc import ABC, abstractmethod
from typing import Any, Callable, Awaitable

from pydantic import BaseModel, Field

from codex_agno_runtime.schemas import RoutingResult, RunTaskResponse

logger = logging.getLogger(__name__)


class MiddlewareContext(BaseModel):
    """Shared context flowing through the middleware pipeline.

    Parameters:
        BaseModel fields are populated by the runtime before chain execution.

    Returns:
        MiddlewareContext: The mutable context object.
    """

    task: str
    session_id: str
    user_id: str
    routing_result: RoutingResult
    prompt: str = ""
    memory_facts: list[str] = Field(default_factory=list)
    active_subagent_count: int = 0
    metadata: dict[str, Any] = Field(default_factory=dict)
    execution_kernel: str | None = None
    execution_kernel_authority: str | None = None
    execution_kernel_delegate: str | None = None
    execution_kernel_delegate_authority: str | None = None


class Middleware(ABC):
    """Base middleware with before/after hooks.

    Subclasses override one or both hooks to inject cross-cutting behavior
    into the agent execution pipeline.
    """

    @property
    def name(self) -> str:
        """Human-readable middleware name.

        Returns:
            str: The class name.
        """
        return self.__class__.__name__

    async def before_agent(self, ctx: MiddlewareContext) -> MiddlewareContext:
        """Run before the agent call.

        Parameters:
            ctx: The middleware context.

        Returns:
            MiddlewareContext: The potentially modified context.
        """
        return ctx

    async def after_agent(
        self, ctx: MiddlewareContext, result: RunTaskResponse
    ) -> RunTaskResponse:
        """Run after the agent call.

        Parameters:
            ctx: The middleware context.
            result: The agent execution result.

        Returns:
            RunTaskResponse: The potentially modified result.
        """
        return result


class MiddlewareChain:
    """Execute an ordered list of middlewares around agent execution.

    Parameters:
        middlewares: Ordered list of middleware instances.
    """

    def __init__(self, middlewares: list[Middleware], trace_recorder: Any | None = None) -> None:
        """Initialize the chain.

        Parameters:
            middlewares: Ordered list of middleware instances.
            trace_recorder: Optional runtime trace recorder.

        Returns:
            None.
        """
        self.middlewares = middlewares
        self._trace = trace_recorder

    def _record_trace_event(
        self,
        *,
        ctx: MiddlewareContext,
        kind: str,
        middleware: Middleware,
        index: int,
        status: str,
        error: Exception | None = None,
    ) -> None:
        """Emit a trace event for middleware lifecycle transitions."""

        if self._trace is None:
            return
        payload: dict[str, Any] = {
            "middleware": middleware.name,
            "index": index,
            "middleware_count": len(self.middlewares),
            "status": status,
        }
        if ctx.execution_kernel is not None:
            payload["execution_kernel"] = ctx.execution_kernel
        if ctx.execution_kernel_authority is not None:
            payload["execution_kernel_authority"] = ctx.execution_kernel_authority
        if ctx.execution_kernel_delegate is not None:
            payload["execution_kernel_delegate"] = ctx.execution_kernel_delegate
        if ctx.execution_kernel_delegate_authority is not None:
            payload["execution_kernel_delegate_authority"] = ctx.execution_kernel_delegate_authority
        if error is not None:
            payload["error"] = str(error)
        self._trace.record(
            session_id=ctx.session_id,
            kind=kind,
            stage="middleware",
            payload=payload,
        )

    def _close_middlewares(
        self,
        *,
        ctx: MiddlewareContext,
        entered: list[tuple[int, Middleware]],
        status: str,
        error: Exception | None = None,
    ) -> None:
        """Emit exit events for middleware that has already entered."""

        for index, middleware in reversed(entered):
            self._record_trace_event(
                ctx=ctx,
                kind="middleware.exit",
                middleware=middleware,
                index=index,
                status=status,
                error=error,
            )

    async def execute(
        self,
        ctx: MiddlewareContext,
        agent_fn: Callable[[MiddlewareContext], Awaitable[RunTaskResponse]],
    ) -> RunTaskResponse:
        """Run the full middleware pipeline.

        Parameters:
            ctx: The initial middleware context.
            agent_fn: The core agent execution function.

        Returns:
            RunTaskResponse: The final result after all after_agent hooks.
        """
        entered: list[tuple[int, Middleware]] = []

        # Before hooks (forward order)
        for mw in self.middlewares:
            logger.debug("Middleware before_agent: %s", mw.name)
            index = len(entered)
            self._record_trace_event(
                ctx=ctx,
                kind="middleware.enter",
                middleware=mw,
                index=index,
                status="ok",
            )
            try:
                ctx = await mw.before_agent(ctx)
            except Exception as error:
                self._record_trace_event(
                    ctx=ctx,
                    kind="middleware.exit",
                    middleware=mw,
                    index=index,
                    status="error",
                    error=error,
                )
                self._close_middlewares(ctx=ctx, entered=entered, status="error", error=error)
                raise
            entered.append((index, mw))

        # Core execution
        try:
            result = await agent_fn(ctx)
        except Exception as error:
            self._close_middlewares(ctx=ctx, entered=entered, status="error", error=error)
            raise

        # After hooks (reverse order for proper unwinding)
        for index, mw in reversed(entered):
            logger.debug("Middleware after_agent: %s", mw.name)
            try:
                result = await mw.after_agent(ctx, result)
            except Exception as error:
                self._record_trace_event(
                    ctx=ctx,
                    kind="middleware.exit",
                    middleware=mw,
                    index=index,
                    status="error",
                    error=error,
                )
                remaining = entered[:entered.index((index, mw))]
                self._close_middlewares(ctx=ctx, entered=remaining, status="error", error=error)
                raise
            self._record_trace_event(
                ctx=ctx,
                kind="middleware.exit",
                middleware=mw,
                index=index,
                status="ok",
            )

        return result


# ---------------------------------------------------------------------------
# Built-in middlewares
# ---------------------------------------------------------------------------


def _python_prompt_required(ctx: MiddlewareContext) -> bool:
    if "python_prompt_required" in ctx.metadata:
        return bool(ctx.metadata["python_prompt_required"])
    if "dry_run" in ctx.metadata:
        return bool(ctx.metadata["dry_run"])
    return True


class SkillInjectionMiddleware(Middleware):
    """Inject skill-based prompt into the context.

    Replaces the inline PromptBuilder call that was previously in runtime.py.
    """

    def __init__(self, prompt_builder: Any) -> None:
        """Initialize with a PromptBuilder instance.

        Parameters:
            prompt_builder: The PromptBuilder used for dynamic injection.

        Returns:
            None.
        """
        self._prompt_builder = prompt_builder

    async def before_agent(self, ctx: MiddlewareContext) -> MiddlewareContext:
        """Build and inject the skill-based prompt.

        Parameters:
            ctx: The middleware context.

        Returns:
            MiddlewareContext: Context with prompt populated.
        """
        if not _python_prompt_required(ctx):
            return ctx
        ctx.prompt = self._prompt_builder.build_prompt(ctx.routing_result)
        return ctx


class ContextCompressionMiddleware(Middleware):
    """Compress context when approaching the token budget.

    Inspired by DeerFlow 2.0's SummarizationMiddleware. When the estimated
    prompt tokens exceed (budget * threshold), older sections are summarized.
    """

    def __init__(self, budget_tokens: int = 80000, threshold: float = 0.75) -> None:
        """Initialize with budget parameters.

        Parameters:
            budget_tokens: Maximum context window budget.
            threshold: Trigger compression at this fraction of budget.

        Returns:
            None.
        """
        self._budget = budget_tokens
        self._threshold = threshold

    async def before_agent(self, ctx: MiddlewareContext) -> MiddlewareContext:
        """Compress the prompt if it exceeds the threshold.

        Parameters:
            ctx: The middleware context.

        Returns:
            MiddlewareContext: Context with potentially compressed prompt.
        """
        if not _python_prompt_required(ctx):
            return ctx
        from codex_agno_runtime.context import ContextEngineer

        estimated = _estimate_tokens(ctx.prompt)
        limit = int(self._budget * self._threshold)
        if estimated > limit:
            logger.info(
                "Context compression triggered: %d tokens > %d limit",
                estimated,
                limit,
            )
            engineer = ContextEngineer()
            ctx.prompt = engineer.estimate_and_compress(ctx.prompt, limit)
        return ctx


class MemoryMiddleware(Middleware):
    """Inject long-term memory facts and extract new facts after the run.

    Inspired by DeerFlow 2.0's persistent memory system.
    """

    def __init__(self, memory_store: Any) -> None:
        """Initialize with a FactMemoryStore.

        Parameters:
            memory_store: The persistent memory store.

        Returns:
            None.
        """
        self._store = memory_store

    async def before_agent(self, ctx: MiddlewareContext) -> MiddlewareContext:
        """Load and inject user memory facts into context.

        Parameters:
            ctx: The middleware context.

        Returns:
            MiddlewareContext: Context with memory facts injected.
        """
        if not _python_prompt_required(ctx):
            return ctx
        facts = self._store.load_facts(ctx.user_id)
        if facts:
            ctx.memory_facts = facts
            memory_block = "\n".join(f"- {f}" for f in facts)
            ctx.prompt = (
                f"[Long-Term Memory]\nThe following facts are known about this user:\n"
                f"{memory_block}\n\n{ctx.prompt}"
            )
            logger.info("Injected %d memory facts for user %s", len(facts), ctx.user_id)
        return ctx

    async def after_agent(
        self, ctx: MiddlewareContext, result: RunTaskResponse
    ) -> RunTaskResponse:
        """Extract and persist new facts from the conversation.

        Parameters:
            ctx: The middleware context.
            result: The agent result.

        Returns:
            RunTaskResponse: The unmodified result.
        """
        if ctx.metadata.get("dry_run"):
            return result
        if result.content:
            conversation = f"User: {ctx.task}\nAssistant: {result.content}"
            # Run blocking fact extraction off the event loop
            new_facts = await asyncio.to_thread(
                self._store.extract_facts_sync, conversation
            )
            if new_facts:
                await asyncio.to_thread(
                    self._store.save_facts, ctx.user_id, new_facts
                )
                logger.info(
                    "Extracted %d new facts for user %s", len(new_facts), ctx.user_id
                )
        return result


class SubagentLimitMiddleware(Middleware):
    """Enforce hard limits on concurrent sub-agents.

    Inspired by DeerFlow 2.0's programmatic enforcement — uses code, not prompts,
    to prevent exceeding the sub-agent limit.
    """

    def __init__(self, max_concurrent: int = 3, timeout_seconds: int = 900) -> None:
        """Initialize with limit parameters.

        Parameters:
            max_concurrent: Maximum concurrent sub-agents.
            timeout_seconds: Timeout per sub-agent in seconds.

        Returns:
            None.
        """
        self._max_concurrent = max_concurrent
        self._timeout = timeout_seconds
        # Lazily initialized inside the event loop to avoid 'no current event loop' errors
        # when the middleware is instantiated outside of an async context.
        self._semaphore: asyncio.Semaphore | None = None
        self._active_count = 0

    def _get_semaphore(self) -> asyncio.Semaphore:
        """Return or create the semaphore within the running event loop.

        Returns:
            asyncio.Semaphore: The rate-limiting semaphore.
        """
        if self._semaphore is None:
            self._semaphore = asyncio.Semaphore(self._max_concurrent)
        return self._semaphore

    @property
    def active_count(self) -> int:
        """Current number of active sub-agents.

        Returns:
            int: Active count.
        """
        return self._active_count

    async def before_agent(self, ctx: MiddlewareContext) -> MiddlewareContext:
        """Track and enforce sub-agent limits.

        Parameters:
            ctx: The middleware context.

        Returns:
            MiddlewareContext: Context with updated sub-agent metadata.
        """
        ctx.active_subagent_count = self._active_count
        ctx.metadata["max_concurrent_subagents"] = self._max_concurrent
        ctx.metadata["subagent_timeout_seconds"] = self._timeout

        if _python_prompt_required(ctx):
            ctx.prompt += (
                f"\n\n[Sub-Agent Limits] Hard limit: {self._max_concurrent} concurrent sub-agents. "
                f"Timeout: {self._timeout}s per sub-agent. "
                f"Currently active: {self._active_count}. "
                f"The system will programmatically enforce these limits."
            )
        return ctx


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _estimate_tokens(text: str) -> int:
    """Quick token estimation (4 chars ≈ 1 token).

    Parameters:
        text: The text to estimate.

    Returns:
        int: Estimated token count.
    """
    if not text:
        return 0
    return max(1, (len(text.strip()) + 3) // 4)
