#!/usr/bin/env python3
"""
Open the Tencent Enterprise Mail compose page in the default browser.

Supports two strategies:
  1. Web compose URL (default): opens exmail.qq.com compose page directly
  2. mailto: fallback: opens the system default mail client

Usage:
  python3 open_compose.py --to prof@uni.edu --subject "..." --body "Hello"
  python3 open_compose.py --to prof@uni.edu --subject "..." --body-file /tmp/body.txt
  python3 open_compose.py --to prof@uni.edu --subject "..." --body "Hello" --mailto
"""

import argparse
import sys
import webbrowser
from pathlib import Path
from urllib.parse import quote, urlencode


# Tencent Enterprise Mail web compose base URL
EXMAIL_COMPOSE_URL = "https://exmail.qq.com/cgi-bin/compose_send"


def _build_web_url(*, to: str, subject: str, body: str, cc: str | None = None) -> str:
    """Build the exmail.qq.com web compose URL with pre-filled fields."""
    params: dict[str, str] = {
        "receivers": to,
        "subject": subject,
        "content": body,
    }
    if cc:
        params["cc"] = cc
    return f"{EXMAIL_COMPOSE_URL}?{urlencode(params)}"


def _build_mailto(*, to: str, subject: str, body: str, cc: str | None = None) -> str:
    """Build a mailto: URI with encoded parameters."""
    uri = f"mailto:{quote(to)}?subject={quote(subject)}&body={quote(body)}"
    if cc:
        uri += f"&cc={quote(cc)}"
    return uri


def open_compose(*, to: str, subject: str, body: str, cc: str | None = None, use_mailto: bool = False) -> bool:
    """
    Open an email compose page in the default browser.

    Args:
        to: Recipient email address.
        subject: Email subject line.
        body: Plain-text email body.
        cc: Optional CC recipient(s).
        use_mailto: If True, use mailto: URI instead of web compose URL.

    Returns:
        True if the browser was opened successfully.
    """
    if use_mailto:
        url = _build_mailto(to=to, subject=subject, body=body, cc=cc)
        method = "mailto"
    else:
        url = _build_web_url(to=to, subject=subject, body=body, cc=cc)
        method = "web"

    try:
        webbrowser.open(url)
        print(f"✓ Compose page opened via {method} — review and send in browser")
        print(f"  To:      {to}")
        print(f"  Subject: {subject}")
        if not use_mailto:
            print(f"  Note:    you may need to log in to exmail.qq.com first")
        return True
    except Exception as e:
        print(f"ERROR: failed to open browser — {e}", file=sys.stderr)
        if not use_mailto:
            print("Hint: try --mailto flag for system default mail client", file=sys.stderr)
        return False


def main() -> None:
    """CLI entry point."""
    parser = argparse.ArgumentParser(
        description="Open email compose page in browser with pre-filled fields.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("--to", required=True, help="Recipient email address")
    parser.add_argument("--subject", required=True, help="Email subject line")

    body_group = parser.add_mutually_exclusive_group(required=True)
    body_group.add_argument("--body", help="Email body text (inline)")
    body_group.add_argument("--body-file", type=Path, help="Read body from file")

    parser.add_argument(
        "--mailto",
        action="store_true",
        help="Use mailto: URI instead of web compose URL (opens system mail client)",
    )
    parser.add_argument(
        "--cc",
        default=None,
        help="CC recipient(s), comma-separated",
    )

    args = parser.parse_args()

    # resolve body
    if args.body_file:
        if not args.body_file.is_file():
            print(f"ERROR: body file not found: {args.body_file}", file=sys.stderr)
            sys.exit(1)
        body = args.body_file.read_text(encoding="utf-8")
    else:
        body = args.body

    ok = open_compose(to=args.to, subject=args.subject, body=body, cc=args.cc, use_mailto=args.mailto)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
