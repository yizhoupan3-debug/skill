#!/usr/bin/env python3
"""Generate or edit images via VibeProxy Local Responses API.

Default path:
- POST http://127.0.0.1:8318/v1/responses
- tools: [{"type": "image_generation"}]

This script keeps the existing command surface (`generate`, `edit`,
`generate-batch`) but no longer depends on the legacy image API or an OpenAI
SDK package.
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import json
import mimetypes
import os
from io import BytesIO
from pathlib import Path
import re
import sys
import time
from typing import Any, Dict, Iterable, List, Optional, Tuple
from urllib import error as urlerror
from urllib import request as urlrequest

DEFAULT_MODEL = "gpt-5.4"
DEFAULT_RESPONSES_URL = "http://127.0.0.1:8318/v1/responses"
DEFAULT_SIZE = "1024x1024"
DEFAULT_QUALITY = "auto"
DEFAULT_OUTPUT_FORMAT = "png"
DEFAULT_CONCURRENCY = 5
DEFAULT_DOWNSCALE_SUFFIX = "-web"
DEFAULT_OUTPUT_PATH = "output/imagegen/output.png"
DEFAULT_TIMEOUT_SECONDS = 300

ALLOWED_SIZES = {"1024x1024", "1536x1024", "1024x1536", "auto"}
ALLOWED_QUALITIES = {"low", "medium", "high", "auto"}
ALLOWED_BACKGROUNDS = {"transparent", "opaque", "auto", None}
ALLOWED_INPUT_FIDELITIES = {"low", "high", None}
TRANSIENT_HTTP_STATUS = {408, 409, 429, 500, 502, 503, 504}

MAX_IMAGE_BYTES = 50 * 1024 * 1024
MAX_BATCH_JOBS = 500


class ResponsesRequestError(RuntimeError):
    def __init__(
        self,
        message: str,
        *,
        status_code: Optional[int] = None,
        retry_after: Optional[float] = None,
    ) -> None:
        super().__init__(message)
        self.status_code = status_code
        self.retry_after = retry_after


def _die(message: str, code: int = 1) -> None:
    print(f"Error: {message}", file=sys.stderr)
    raise SystemExit(code)


def _warn(message: str) -> None:
    print(f"Warning: {message}", file=sys.stderr)


def _responses_url() -> str:
    return os.getenv("VIBEPROXY_RESPONSES_URL", DEFAULT_RESPONSES_URL)


def _auth_headers() -> Dict[str, str]:
    for env_name in ("VIBEPROXY_BEARER_TOKEN", "VIBEPROXY_API_KEY"):
        token = os.getenv(env_name)
        if token:
            return {"Authorization": f"Bearer {token}"}
    return {}


def _read_prompt(prompt: Optional[str], prompt_file: Optional[str]) -> str:
    if prompt and prompt_file:
        _die("Use --prompt or --prompt-file, not both.")
    if prompt_file:
        path = Path(prompt_file)
        if not path.exists():
            _die(f"Prompt file not found: {path}")
        return path.read_text(encoding="utf-8").strip()
    if prompt:
        return prompt.strip()
    _die("Missing prompt. Use --prompt or --prompt-file.")
    return ""


def _check_image_paths(paths: Iterable[str]) -> List[Path]:
    resolved: List[Path] = []
    for raw in paths:
        path = Path(raw)
        if not path.exists():
            _die(f"Image file not found: {path}")
        if path.stat().st_size > MAX_IMAGE_BYTES:
            _warn(f"Image exceeds 50MB limit: {path}")
        resolved.append(path)
    return resolved


def _normalize_output_format(fmt: Optional[str]) -> str:
    if not fmt:
        return DEFAULT_OUTPUT_FORMAT
    fmt = fmt.lower()
    if fmt not in {"png", "jpeg", "jpg", "webp"}:
        _die("output-format must be png, jpeg, jpg, or webp.")
    return "jpeg" if fmt == "jpg" else fmt


def _validate_size(size: str) -> None:
    if size not in ALLOWED_SIZES:
        _die("size must be one of 1024x1024, 1536x1024, 1024x1536, or auto.")


def _validate_quality(quality: str) -> None:
    if quality not in ALLOWED_QUALITIES:
        _die("quality must be one of low, medium, high, or auto.")


def _validate_background(background: Optional[str]) -> None:
    if background not in ALLOWED_BACKGROUNDS:
        _die("background must be one of transparent, opaque, or auto.")


def _validate_input_fidelity(input_fidelity: Optional[str]) -> None:
    if input_fidelity not in ALLOWED_INPUT_FIDELITIES:
        _die("input-fidelity must be one of low or high.")


def _validate_transparency(background: Optional[str], output_format: str) -> None:
    if background == "transparent" and output_format not in {"png", "webp"}:
        _die("transparent background requires output-format png or webp.")


def _build_output_paths(
    out: str,
    output_format: str,
    count: int,
    out_dir: Optional[str],
) -> List[Path]:
    ext = "." + output_format

    if out_dir:
        out_base = Path(out_dir)
        out_base.mkdir(parents=True, exist_ok=True)
        return [out_base / f"image_{i}{ext}" for i in range(1, count + 1)]

    out_path = Path(out)
    if out_path.exists() and out_path.is_dir():
        out_path.mkdir(parents=True, exist_ok=True)
        return [out_path / f"image_{i}{ext}" for i in range(1, count + 1)]

    if out_path.suffix == "":
        out_path = out_path.with_suffix(ext)
    elif out_path.suffix.lstrip(".").lower() != output_format:
        _warn(
            f"Output extension {out_path.suffix} does not match output-format {output_format}."
        )

    if count == 1:
        return [out_path]

    return [
        out_path.with_name(f"{out_path.stem}-{i}{out_path.suffix}")
        for i in range(1, count + 1)
    ]


def _augment_prompt(args: argparse.Namespace, prompt: str) -> str:
    fields = _fields_from_args(args)
    return _augment_prompt_fields(args.augment, prompt, fields)


def _augment_prompt_fields(augment: bool, prompt: str, fields: Dict[str, Optional[str]]) -> str:
    if not augment:
        return prompt

    sections: List[str] = []
    if fields.get("use_case"):
        sections.append(f"Use case: {fields['use_case']}")
    sections.append(f"Primary request: {prompt}")
    if fields.get("scene"):
        sections.append(f"Scene/backdrop: {fields['scene']}")
    if fields.get("subject"):
        sections.append(f"Subject: {fields['subject']}")
    if fields.get("style"):
        sections.append(f"Style/medium: {fields['style']}")
    if fields.get("composition"):
        sections.append(f"Composition/framing: {fields['composition']}")
    if fields.get("lighting"):
        sections.append(f"Lighting/mood: {fields['lighting']}")
    if fields.get("palette"):
        sections.append(f"Color palette: {fields['palette']}")
    if fields.get("materials"):
        sections.append(f"Materials/textures: {fields['materials']}")
    if fields.get("text"):
        sections.append(f"Text (verbatim): \"{fields['text']}\"")
    if fields.get("constraints"):
        sections.append(f"Constraints: {fields['constraints']}")
    if fields.get("negative"):
        sections.append(f"Avoid: {fields['negative']}")
    return "\n".join(sections)


def _fields_from_args(args: argparse.Namespace) -> Dict[str, Optional[str]]:
    return {
        "use_case": getattr(args, "use_case", None),
        "scene": getattr(args, "scene", None),
        "subject": getattr(args, "subject", None),
        "style": getattr(args, "style", None),
        "composition": getattr(args, "composition", None),
        "lighting": getattr(args, "lighting", None),
        "palette": getattr(args, "palette", None),
        "materials": getattr(args, "materials", None),
        "text": getattr(args, "text", None),
        "constraints": getattr(args, "constraints", None),
        "negative": getattr(args, "negative", None),
    }


def _print_request(payload: dict) -> None:
    print(json.dumps(payload, indent=2, ensure_ascii=False, sort_keys=True))


def _decode_write_and_downscale(
    images: List[str],
    outputs: List[Path],
    *,
    force: bool,
    downscale_max_dim: Optional[int],
    downscale_suffix: str,
    output_format: str,
) -> None:
    for idx, image_b64 in enumerate(images):
        if idx >= len(outputs):
            break
        out_path = outputs[idx]
        if out_path.exists() and not force:
            _die(f"Output already exists: {out_path} (use --force to overwrite)")
        out_path.parent.mkdir(parents=True, exist_ok=True)

        raw = base64.b64decode(image_b64)
        out_path.write_bytes(raw)
        print(f"Wrote {out_path}")

        if downscale_max_dim is None:
            continue

        derived = _derive_downscale_path(out_path, downscale_suffix)
        if derived.exists() and not force:
            _die(f"Output already exists: {derived} (use --force to overwrite)")
        derived.parent.mkdir(parents=True, exist_ok=True)
        resized = _downscale_image_bytes(raw, max_dim=downscale_max_dim, output_format=output_format)
        derived.write_bytes(resized)
        print(f"Wrote {derived}")


def _derive_downscale_path(path: Path, suffix: str) -> Path:
    if suffix and not suffix.startswith("-") and not suffix.startswith("_"):
        suffix = "-" + suffix
    return path.with_name(f"{path.stem}{suffix}{path.suffix}")


def _downscale_image_bytes(image_bytes: bytes, *, max_dim: int, output_format: str) -> bytes:
    try:
        from PIL import Image
    except Exception:
        _die("Downscaling requires Pillow. Install with `uv pip install pillow`.")

    if max_dim < 1:
        _die("--downscale-max-dim must be >= 1")

    with Image.open(BytesIO(image_bytes)) as img:
        img.load()
        w, h = img.size
        scale = min(1.0, float(max_dim) / float(max(w, h)))
        target = (max(1, int(round(w * scale))), max(1, int(round(h * scale))))
        resized = img if target == (w, h) else img.resize(target, Image.Resampling.LANCZOS)

        fmt = output_format.lower()
        if fmt == "jpg":
            fmt = "jpeg"
        if fmt == "jpeg":
            if resized.mode in ("RGBA", "LA") or "transparency" in getattr(resized, "info", {}):
                bg = Image.new("RGB", resized.size, (255, 255, 255))
                bg.paste(resized.convert("RGBA"), mask=resized.convert("RGBA").split()[-1])
                resized = bg
            else:
                resized = resized.convert("RGB")

        out = BytesIO()
        resized.save(out, format=fmt.upper())
        return out.getvalue()


def _slugify(value: str) -> str:
    value = value.strip().lower()
    value = re.sub(r"[^a-z0-9]+", "-", value)
    value = re.sub(r"-{2,}", "-", value).strip("-")
    return value[:60] if value else "job"


def _normalize_job(job: Any, idx: int) -> Dict[str, Any]:
    if isinstance(job, str):
        prompt = job.strip()
        if not prompt:
            _die(f"Empty prompt at job {idx}")
        return {"prompt": prompt}
    if isinstance(job, dict):
        if "prompt" not in job or not str(job["prompt"]).strip():
            _die(f"Missing prompt for job {idx}")
        return job
    _die(f"Invalid job at index {idx}: expected string or object.")
    return {}


def _read_jobs_jsonl(path: str) -> List[Dict[str, Any]]:
    p = Path(path)
    if not p.exists():
        _die(f"Input file not found: {p}")
    jobs: List[Dict[str, Any]] = []
    for line_no, raw in enumerate(p.read_text(encoding="utf-8").splitlines(), start=1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        try:
            item: Any = json.loads(line) if line.startswith("{") else line
        except json.JSONDecodeError as exc:
            _die(f"Invalid JSON on line {line_no}: {exc}")
        jobs.append(_normalize_job(item, idx=line_no))
    if not jobs:
        _die("No jobs found in input file.")
    if len(jobs) > MAX_BATCH_JOBS:
        _die(f"Too many jobs ({len(jobs)}). Max is {MAX_BATCH_JOBS}.")
    return jobs


def _merge_non_null(dst: Dict[str, Any], src: Dict[str, Any]) -> Dict[str, Any]:
    merged = dict(dst)
    for key, value in src.items():
        if value is not None:
            merged[key] = value
    return merged


def _job_output_paths(
    *,
    out_dir: Path,
    output_format: str,
    idx: int,
    prompt: str,
    n: int,
    explicit_out: Optional[str],
) -> List[Path]:
    out_dir.mkdir(parents=True, exist_ok=True)
    ext = "." + output_format

    if explicit_out:
        base = Path(explicit_out)
        if base.suffix == "":
            base = base.with_suffix(ext)
        elif base.suffix.lstrip(".").lower() != output_format:
            _warn(
                f"Job {idx}: output extension {base.suffix} does not match output-format {output_format}."
            )
        base = out_dir / base.name
    else:
        slug = _slugify(prompt[:80])
        base = out_dir / f"{idx:03d}-{slug}{ext}"

    if n == 1:
        return [base]
    return [base.with_name(f"{base.stem}-{i}{base.suffix}") for i in range(1, n + 1)]


def _guess_mime_type(path: Path) -> str:
    guessed, _ = mimetypes.guess_type(path.name)
    if guessed:
        return guessed
    suffix = path.suffix.lower()
    if suffix in {".jpg", ".jpeg"}:
        return "image/jpeg"
    if suffix == ".webp":
        return "image/webp"
    if suffix == ".gif":
        return "image/gif"
    return "image/png"


def _image_to_data_url(path: Path) -> str:
    mime = _guess_mime_type(path)
    encoded = base64.b64encode(path.read_bytes()).decode("ascii")
    return f"data:{mime};base64,{encoded}"


def _preview_data_url(path: Path) -> str:
    return f"data:{_guess_mime_type(path)};base64,<omitted:{path.name}>"


def _build_tool(args: argparse.Namespace, *, action: Optional[str], include_input_fidelity: bool) -> Dict[str, Any]:
    tool: Dict[str, Any] = {"type": "image_generation"}
    if args.size is not None:
        tool["size"] = args.size
    if args.quality is not None:
        tool["quality"] = args.quality
    if args.background is not None:
        tool["background"] = args.background
    if args.output_format is not None:
        tool["output_format"] = _normalize_output_format(args.output_format)
    if args.output_compression is not None:
        tool["output_compression"] = args.output_compression
    if args.moderation is not None:
        tool["moderation"] = args.moderation
    if action is not None:
        tool["action"] = action
    if include_input_fidelity and args.input_fidelity is not None:
        tool["input_fidelity"] = args.input_fidelity
    return tool


def _build_generate_payload(args: argparse.Namespace, prompt: str) -> Dict[str, Any]:
    return {
        "model": args.model,
        "input": prompt,
        "tools": [_build_tool(args, action=None, include_input_fidelity=False)],
    }


def _build_generate_preview_payload(args: argparse.Namespace, prompt: str) -> Dict[str, Any]:
    return _build_generate_payload(args, prompt)


def _build_edit_payload(args: argparse.Namespace, prompt: str, image_paths: List[Path]) -> Dict[str, Any]:
    content: List[Dict[str, Any]] = [{"type": "input_text", "text": prompt}]
    content.extend({"type": "input_image", "image_url": _image_to_data_url(path)} for path in image_paths)
    return {
        "model": args.model,
        "input": [{"role": "user", "content": content}],
        "tools": [_build_tool(args, action="edit", include_input_fidelity=True)],
    }


def _build_edit_preview_payload(
    args: argparse.Namespace,
    prompt: str,
    image_paths: List[Path],
) -> Dict[str, Any]:
    content: List[Dict[str, Any]] = [{"type": "input_text", "text": prompt}]
    content.extend(
        {"type": "input_image", "image_url": _preview_data_url(path)} for path in image_paths
    )
    return {
        "model": args.model,
        "input": [{"role": "user", "content": content}],
        "tools": [_build_tool(args, action="edit", include_input_fidelity=True)],
    }


def _parse_retry_after(header_value: Optional[str]) -> Optional[float]:
    if not header_value:
        return None
    try:
        return float(header_value)
    except ValueError:
        return None


def _post_responses_request(payload: Dict[str, Any]) -> Dict[str, Any]:
    body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    headers = {"Content-Type": "application/json", "Accept": "application/json"}
    headers.update(_auth_headers())
    req = urlrequest.Request(
        _responses_url(),
        data=body,
        headers=headers,
        method="POST",
    )
    try:
        with urlrequest.urlopen(req, timeout=DEFAULT_TIMEOUT_SECONDS) as resp:
            raw = resp.read().decode("utf-8")
            return json.loads(raw)
    except urlerror.HTTPError as exc:
        raw = exc.read().decode("utf-8", errors="replace")
        retry_after = _parse_retry_after(exc.headers.get("Retry-After"))
        raise ResponsesRequestError(
            f"HTTP {exc.code} from {_responses_url()}: {raw}",
            status_code=exc.code,
            retry_after=retry_after,
        ) from exc
    except urlerror.URLError as exc:
        raise ResponsesRequestError(f"Failed to reach {_responses_url()}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise ResponsesRequestError(f"Invalid JSON returned by {_responses_url()}: {exc}") from exc


def _is_transient_error(exc: Exception) -> bool:
    if isinstance(exc, ResponsesRequestError):
        if exc.status_code in TRANSIENT_HTTP_STATUS:
            return True
        if exc.status_code is None:
            return True
    message = str(exc).lower()
    return "timed out" in message or "timeout" in message or "connection reset" in message


def _retry_after_seconds(exc: Exception, attempt: int) -> float:
    if isinstance(exc, ResponsesRequestError) and exc.retry_after is not None:
        return exc.retry_after
    return min(60.0, 2.0**attempt)


def _request_with_retries(
    payload: Dict[str, Any],
    *,
    attempts: int,
    label: str,
) -> Dict[str, Any]:
    last_exc: Optional[Exception] = None
    for attempt in range(1, attempts + 1):
        try:
            return _post_responses_request(payload)
        except Exception as exc:
            last_exc = exc
            if not _is_transient_error(exc) or attempt == attempts:
                raise
            sleep_s = _retry_after_seconds(exc, attempt)
            print(
                f"{label} attempt {attempt}/{attempts} failed ({exc}); retrying in {sleep_s:.1f}s",
                file=sys.stderr,
            )
            time.sleep(sleep_s)
    raise last_exc or RuntimeError("unknown request failure")


def _extract_generated_images(response: Dict[str, Any]) -> List[str]:
    output = response.get("output")
    if not isinstance(output, list):
        raise ResponsesRequestError("Responses payload missing `output` list.")

    images: List[str] = []
    for item in output:
        if not isinstance(item, dict):
            continue
        if item.get("type") != "image_generation_call":
            continue
        result = item.get("result")
        if isinstance(result, str) and result:
            images.append(result)
        elif isinstance(result, list):
            images.extend(part for part in result if isinstance(part, str) and part)

    if images:
        return images

    raise ResponsesRequestError(
        "Responses call completed without an `image_generation_call` result. "
        f"Observed output types: {[item.get('type') for item in output if isinstance(item, dict)]}"
    )


def _generate_many(
    args: argparse.Namespace,
    prompt: str,
    output_paths: List[Path],
    *,
    job_label: str,
) -> None:
    output_format = _normalize_output_format(args.output_format)
    for index, out_path in enumerate(output_paths, start=1):
        label = job_label
        if len(output_paths) > 1:
            label = f"{job_label} variant {index}/{len(output_paths)}"
        print(f"{label} -> POST {_responses_url()}", file=sys.stderr)
        started = time.time()
        response = _request_with_retries(
            _build_generate_payload(args, prompt),
            attempts=getattr(args, "max_attempts", 3),
            label=label,
        )
        elapsed = time.time() - started
        print(f"{label} completed in {elapsed:.1f}s", file=sys.stderr)
        images = _extract_generated_images(response)
        _decode_write_and_downscale(
            images[:1],
            [out_path],
            force=args.force,
            downscale_max_dim=args.downscale_max_dim,
            downscale_suffix=args.downscale_suffix,
            output_format=output_format,
        )


def _edit_many(
    args: argparse.Namespace,
    prompt: str,
    image_paths: List[Path],
    output_paths: List[Path],
    *,
    job_label: str,
) -> None:
    if args.mask:
        _die(
            "--mask is not supported on the VibeProxy /v1/responses path yet. "
            "This script currently supports text+image edit via input_image, not input_image_mask/file upload."
        )

    output_format = _normalize_output_format(args.output_format)
    for index, out_path in enumerate(output_paths, start=1):
        label = job_label
        if len(output_paths) > 1:
            label = f"{job_label} variant {index}/{len(output_paths)}"
        print(f"{label} -> POST {_responses_url()} (edit with {len(image_paths)} image(s))", file=sys.stderr)
        started = time.time()
        response = _request_with_retries(
            _build_edit_payload(args, prompt, image_paths),
            attempts=getattr(args, "max_attempts", 3),
            label=label,
        )
        elapsed = time.time() - started
        print(f"{label} completed in {elapsed:.1f}s", file=sys.stderr)
        images = _extract_generated_images(response)
        _decode_write_and_downscale(
            images[:1],
            [out_path],
            force=args.force,
            downscale_max_dim=args.downscale_max_dim,
            downscale_suffix=args.downscale_suffix,
            output_format=output_format,
        )


async def _run_generate_batch(args: argparse.Namespace) -> int:
    jobs = _read_jobs_jsonl(args.input)
    out_dir = Path(args.out_dir)

    base_fields = _fields_from_args(args)
    any_failed = False
    sem = asyncio.Semaphore(args.concurrency)

    if args.dry_run:
        for i, job in enumerate(jobs, start=1):
            job_args = argparse.Namespace(**vars(args))
            prompt = str(job["prompt"]).strip()
            fields = _merge_non_null(base_fields, job.get("fields", {}))
            fields = _merge_non_null(fields, {k: job.get(k) for k in base_fields.keys()})
            augmented = _augment_prompt_fields(args.augment, prompt, fields)

            for key in ("model", "size", "quality", "background", "output_format", "output_compression", "moderation", "n"):
                if key in job and job[key] is not None:
                    setattr(job_args, key.replace("-", "_"), job[key])

            effective_output_format = _normalize_output_format(job_args.output_format)
            _validate_transparency(job_args.background, effective_output_format)
            n = int(job_args.n)
            outputs = _job_output_paths(
                out_dir=out_dir,
                output_format=effective_output_format,
                idx=i,
                prompt=prompt,
                n=n,
                explicit_out=job.get("out"),
            )
            downscaled = None
            if args.downscale_max_dim is not None:
                downscaled = [str(_derive_downscale_path(p, args.downscale_suffix)) for p in outputs]
            _print_request(
                {
                    "endpoint": _responses_url(),
                    "job": i,
                    "outputs": [str(p) for p in outputs],
                    "outputs_downscaled": downscaled,
                    **_build_generate_preview_payload(job_args, augmented),
                }
            )
        return 0

    async def run_job(i: int, job: Dict[str, Any]) -> Tuple[int, Optional[str]]:
        nonlocal any_failed

        job_args = argparse.Namespace(**vars(args))
        prompt = str(job["prompt"]).strip()
        job_label = f"[job {i}/{len(jobs)}]"

        fields = _merge_non_null(base_fields, job.get("fields", {}))
        fields = _merge_non_null(fields, {k: job.get(k) for k in base_fields.keys()})
        augmented = _augment_prompt_fields(args.augment, prompt, fields)

        for key in ("model", "size", "quality", "background", "output_format", "output_compression", "moderation", "n"):
            if key in job and job[key] is not None:
                setattr(job_args, key.replace("-", "_"), job[key])

        effective_output_format = _normalize_output_format(job_args.output_format)
        _validate_transparency(job_args.background, effective_output_format)
        outputs = _job_output_paths(
            out_dir=out_dir,
            output_format=effective_output_format,
            idx=i,
            prompt=prompt,
            n=int(job_args.n),
            explicit_out=job.get("out"),
        )

        try:
            async with sem:
                await asyncio.to_thread(
                    _generate_many,
                    job_args,
                    augmented,
                    outputs,
                    job_label=job_label,
                )
            return i, None
        except Exception as exc:
            any_failed = True
            print(f"{job_label} failed: {exc}", file=sys.stderr)
            if args.fail_fast:
                raise
            return i, str(exc)

    tasks = [asyncio.create_task(run_job(i, job)) for i, job in enumerate(jobs, start=1)]
    try:
        await asyncio.gather(*tasks)
    except Exception:
        for task in tasks:
            if not task.done():
                task.cancel()
        raise
    return 1 if any_failed else 0


def _generate_batch(args: argparse.Namespace) -> None:
    exit_code = asyncio.run(_run_generate_batch(args))
    if exit_code:
        raise SystemExit(exit_code)


def _generate(args: argparse.Namespace) -> None:
    prompt = _augment_prompt(args, _read_prompt(args.prompt, args.prompt_file))
    output_format = _normalize_output_format(args.output_format)
    _validate_transparency(args.background, output_format)
    output_paths = _build_output_paths(args.out, output_format, args.n, args.out_dir)

    if args.dry_run:
        preview = _build_generate_preview_payload(args, prompt)
        preview["endpoint"] = _responses_url()
        preview["outputs"] = [str(path) for path in output_paths]
        _print_request(preview)
        return

    _generate_many(args, prompt, output_paths, job_label="[generate]")


def _edit(args: argparse.Namespace) -> None:
    prompt = _augment_prompt(args, _read_prompt(args.prompt, args.prompt_file))
    image_paths = _check_image_paths(args.image)
    output_format = _normalize_output_format(args.output_format)
    _validate_transparency(args.background, output_format)
    output_paths = _build_output_paths(args.out, output_format, args.n, args.out_dir)

    if args.mask:
        _die(
            "--mask is not supported on the VibeProxy /v1/responses path yet. "
            "This script currently supports text+image edit via input_image, not input_image_mask/file upload."
        )

    if args.dry_run:
        preview = _build_edit_preview_payload(args, prompt, image_paths)
        preview["endpoint"] = _responses_url()
        preview["outputs"] = [str(path) for path in output_paths]
        _print_request(preview)
        return

    _edit_many(args, prompt, image_paths, output_paths, job_label="[edit]")


def _add_shared_args(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--prompt")
    parser.add_argument("--prompt-file")
    parser.add_argument("--n", type=int, default=1)
    parser.add_argument("--size", default=DEFAULT_SIZE)
    parser.add_argument("--quality", default=DEFAULT_QUALITY)
    parser.add_argument("--background")
    parser.add_argument("--output-format")
    parser.add_argument("--output-compression", type=int)
    parser.add_argument("--moderation")
    parser.add_argument("--out", default=DEFAULT_OUTPUT_PATH)
    parser.add_argument("--out-dir")
    parser.add_argument("--force", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--augment", dest="augment", action="store_true")
    parser.add_argument("--no-augment", dest="augment", action="store_false")
    parser.set_defaults(augment=True)

    parser.add_argument("--use-case")
    parser.add_argument("--scene")
    parser.add_argument("--subject")
    parser.add_argument("--style")
    parser.add_argument("--composition")
    parser.add_argument("--lighting")
    parser.add_argument("--palette")
    parser.add_argument("--materials")
    parser.add_argument("--text")
    parser.add_argument("--constraints")
    parser.add_argument("--negative")

    parser.add_argument("--downscale-max-dim", type=int)
    parser.add_argument("--downscale-suffix", default=DEFAULT_DOWNSCALE_SUFFIX)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate or edit images via VibeProxy Local /v1/responses",
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    gen_parser = subparsers.add_parser("generate", help="Create a new image")
    _add_shared_args(gen_parser)
    gen_parser.set_defaults(func=_generate)

    batch_parser = subparsers.add_parser(
        "generate-batch",
        help="Generate multiple prompts concurrently from a JSONL file",
    )
    _add_shared_args(batch_parser)
    batch_parser.add_argument("--input", required=True, help="Path to JSONL file (one job per line)")
    batch_parser.add_argument("--concurrency", type=int, default=DEFAULT_CONCURRENCY)
    batch_parser.add_argument("--max-attempts", type=int, default=3)
    batch_parser.add_argument("--fail-fast", action="store_true")
    batch_parser.set_defaults(func=_generate_batch)

    edit_parser = subparsers.add_parser("edit", help="Edit an existing image")
    _add_shared_args(edit_parser)
    edit_parser.add_argument("--image", action="append", required=True)
    edit_parser.add_argument("--mask")
    edit_parser.add_argument("--input-fidelity")
    edit_parser.add_argument("--max-attempts", type=int, default=3)
    edit_parser.set_defaults(func=_edit)

    args = parser.parse_args()
    if args.n < 1 or args.n > 10:
        _die("--n must be between 1 and 10")
    if getattr(args, "concurrency", 1) < 1 or getattr(args, "concurrency", 1) > 25:
        _die("--concurrency must be between 1 and 25")
    if getattr(args, "max_attempts", 3) < 1 or getattr(args, "max_attempts", 3) > 10:
        _die("--max-attempts must be between 1 and 10")
    if args.output_compression is not None and not (0 <= args.output_compression <= 100):
        _die("--output-compression must be between 0 and 100")
    if args.command == "generate-batch" and not args.out_dir:
        _die("generate-batch requires --out-dir")
    if getattr(args, "downscale_max_dim", None) is not None and args.downscale_max_dim < 1:
        _die("--downscale-max-dim must be >= 1")

    _validate_size(args.size)
    _validate_quality(args.quality)
    _validate_background(args.background)
    _validate_input_fidelity(getattr(args, "input_fidelity", None))
    _validate_transparency(args.background, _normalize_output_format(args.output_format))

    args.func(args)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
