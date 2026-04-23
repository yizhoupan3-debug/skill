"""Deterministic context compression helpers."""

from __future__ import annotations

from dataclasses import dataclass
import re

from framework_runtime.utils import estimate_tokens


@dataclass(frozen=True)
class CompressionResult:
    """Versioned deterministic compression contract."""

    schema_version: str
    prompt: str
    input_token_estimate: int
    output_token_estimate: int
    omitted_sections: int
    strategy: str
    truncated: bool
    artifact_offload_decision: bool


class ContextEngineer:
    """Apply deterministic prompt compression before model execution."""

    def estimate_and_compress(self, prompt: str, token_limit: int) -> str:
        """Compress a prompt to fit within a token budget.

        Parameters:
            prompt: Original prompt text.
            token_limit: Maximum estimated token count.

        Returns:
            str: Compressed prompt.
        """

        return self.compress_contract(prompt, token_limit).prompt

    def compress_contract(self, prompt: str, token_limit: int) -> CompressionResult:
        """Return the stable compression contract for one prompt."""

        input_tokens = estimate_tokens(prompt)
        if input_tokens <= token_limit:
            return CompressionResult(
                schema_version="runtime-compression-v1",
                prompt=prompt,
                input_token_estimate=input_tokens,
                output_token_estimate=input_tokens,
                omitted_sections=0,
                strategy="none",
                truncated=False,
                artifact_offload_decision=False,
            )

        original_sections = [chunk.strip() for chunk in prompt.split("\n\n") if chunk.strip()]
        sections = self._dedupe_sections(prompt)
        deduped = len(sections) != len(original_sections)
        if len(sections) <= 1:
            deduped_prompt = sections[0] if sections else prompt
            return self._truncate_contract(
                deduped_prompt,
                token_limit,
                input_tokens=input_tokens,
                omitted_sections=0,
                strategy="dedupe+truncate" if deduped else "truncate",
            )
        deduped_prompt = "\n\n".join(sections)
        deduped_tokens = estimate_tokens(deduped_prompt)
        if deduped and deduped_tokens <= token_limit:
            return CompressionResult(
                schema_version="runtime-compression-v1",
                prompt=deduped_prompt,
                input_token_estimate=input_tokens,
                output_token_estimate=deduped_tokens,
                omitted_sections=0,
                strategy="dedupe",
                truncated=False,
                artifact_offload_decision=False,
            )

        if len(sections) <= 4:
            return self._truncate_contract(
                deduped_prompt,
                token_limit,
                input_tokens=input_tokens,
                omitted_sections=0,
                strategy="dedupe+truncate" if deduped else "truncate",
            )

        head = sections[:3]
        tail = sections[-2:]
        omitted = max(0, len(sections) - len(head) - len(tail))
        compressed = "\n\n".join(
            [
                *head,
                f"[Context compression]\nOmitted {omitted} middle sections to respect the runtime token budget.",
                *tail,
            ]
        )
        if estimate_tokens(compressed) <= token_limit:
            return CompressionResult(
                schema_version="runtime-compression-v1",
                prompt=compressed,
                input_token_estimate=input_tokens,
                output_token_estimate=estimate_tokens(compressed),
                omitted_sections=omitted,
                strategy="dedupe+head-tail" if deduped else "head-tail",
                truncated=False,
                artifact_offload_decision=False,
            )
        return self._truncate_contract(
            compressed,
            token_limit,
            input_tokens=input_tokens,
            omitted_sections=omitted,
            strategy="dedupe+head-tail+truncate" if deduped else "head-tail+truncate",
        )

    @staticmethod
    def _normalize_section(section: str) -> str:
        lines = [line.strip() for line in section.splitlines() if line.strip()]
        normalized = "\n".join(lines)
        normalized = re.sub(r"\s+", " ", normalized)
        if normalized.startswith("How to reply:"):
            return "style::" + normalized
        return normalized

    def _dedupe_sections(self, prompt: str) -> list[str]:
        sections = [chunk.strip() for chunk in prompt.split("\n\n") if chunk.strip()]
        seen: set[str] = set()
        unique_sections: list[str] = []
        for section in sections:
            key = self._normalize_section(section)
            if key in seen:
                continue
            seen.add(key)
            unique_sections.append(section)
        return unique_sections

    @staticmethod
    def _truncate_contract(
        text: str,
        token_limit: int,
        *,
        input_tokens: int,
        omitted_sections: int,
        strategy: str,
    ) -> CompressionResult:
        """Fallback truncation for already-small structured prompts."""

        omission_prompt = "[Context compression]\nPrompt omitted due to zero token budget."
        truncation_marker = "\n\n[Context compression]\nPrompt tail truncated to fit the token budget."
        if token_limit <= 0:
            prompt = omission_prompt
            return CompressionResult(
                schema_version="runtime-compression-v1",
                prompt=prompt,
                input_token_estimate=input_tokens,
                output_token_estimate=estimate_tokens(prompt),
                omitted_sections=omitted_sections,
                strategy=strategy,
                truncated=True,
                artifact_offload_decision=False,
            )

        words = text.split()
        estimated_words = max(16, token_limit * 2)
        candidate = text if len(words) <= estimated_words else " ".join(words[:estimated_words])
        if estimate_tokens(candidate) <= token_limit:
            return CompressionResult(
                schema_version="runtime-compression-v1",
                prompt=candidate,
                input_token_estimate=input_tokens,
                output_token_estimate=estimate_tokens(candidate),
                omitted_sections=omitted_sections,
                strategy=strategy,
                truncated=False,
                artifact_offload_decision=False,
            )

        if estimate_tokens(truncation_marker) >= token_limit:
            prompt = omission_prompt
            return CompressionResult(
                schema_version="runtime-compression-v1",
                prompt=prompt,
                input_token_estimate=input_tokens,
                output_token_estimate=estimate_tokens(prompt),
                omitted_sections=omitted_sections,
                strategy=strategy,
                truncated=True,
                artifact_offload_decision=False,
            )

        while candidate and estimate_tokens(candidate + truncation_marker) > token_limit:
            candidate_words = candidate.split()
            if len(candidate_words) > 1:
                candidate = " ".join(candidate_words[:-1])
            else:
                candidate = candidate[:-1].strip()

        prompt = candidate + truncation_marker if candidate else omission_prompt
        return CompressionResult(
            schema_version="runtime-compression-v1",
            prompt=prompt,
            input_token_estimate=input_tokens,
            output_token_estimate=estimate_tokens(prompt),
            omitted_sections=omitted_sections,
            strategy=strategy,
            truncated=True,
            artifact_offload_decision=False,
        )
