"""Pluggable middleware chain for the Codex Agno runtime."""

from __future__ import annotations

import asyncio
import logging
from abc import ABC
from typing import Any, Callable, Awaitable

from pydantic import BaseModel, Field

from framework_runtime.schemas import RoutingResult, RunTaskResponse
from framework_runtime.utils import estimate_tokens

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
    job_id: str | None = None
    user_id: str
    routing_result: RoutingResult
    prompt: str = ""
    memory_facts: list[str] = Field(default_factory=list)
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
        if not ctx.prompt:
            return ctx
        from framework_runtime.context import ContextEngineer

        estimated = estimate_tokens(ctx.prompt)
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
        facts = self._store.load_facts(ctx.user_id)
        if facts:
            ctx.memory_facts = facts
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


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
