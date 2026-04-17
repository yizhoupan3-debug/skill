#!/usr/bin/env python3
"""
SMTP email sender for SUSTech student mailbox.

Sends emails via Tencent Enterprise Mail (smtp.exmail.qq.com:465 SSL)
using the SUSTech student mailbox 12312411@mail.sustech.edu.cn.

Supports two modes:
  - default: English professional emails with CV/transcript attachments
  - homework: Chinese assignment submission emails (no CV/transcript)

Credentials are read from:
  1. Environment variables SUSTECH_SMTP_USER / SUSTECH_SMTP_PASS
     (legacy: TAOXI_SMTP_USER / TAOXI_SMTP_PASS)
  2. ~/.sustech-mailer-smtp.env (fallback: ~/.tao-ci-smtp.env)

Usage:
  python3 send_email.py --to prof@uni.edu --subject "..." --body "..."
  python3 send_email.py --to ta@uni.edu --subject "..." --body-file /tmp/body.txt --mode homework
  python3 send_email.py --to prof@uni.edu --subject "..." --body "..." --dry-run
  python3 send_email.py --to prof@uni.edu --subject "..." --body "..." --no-attachments
"""

import argparse
import os
import smtplib
import ssl
import sys
from email import encoders
from email.mime.base import MIMEBase
from email.mime.multipart import MIMEMultipart
from email.mime.text import MIMEText
from email.utils import encode_rfc2231
from pathlib import Path

# ── defaults ────────────────────────────────────────────────────────────────

SMTP_SERVER = "smtp.exmail.qq.com"
SMTP_PORT = 465
DEFAULT_SENDER = "12312411@mail.sustech.edu.cn"
SENDER_DISPLAY = "Yizhou Pan"

# Credential file paths (checked in order)
ENV_FILES = [
    Path.home() / ".sustech-mailer-smtp.env",
    Path.home() / ".tao-ci-smtp.env",
]

# Default attachments for professor outreach (default mode)
DEFAULT_ATTACHMENTS = [
    Path("/Users/joe/Documents/套磁/CV_Yizhou_Pan.pdf"),
    Path("/Users/joe/Documents/套磁/Academic transcript.pdf"),
]


# ── helpers ─────────────────────────────────────────────────────────────────


def _load_env_file(path: Path) -> dict[str, str]:
    """Parse a simple KEY=VALUE env file, ignoring comments and blanks."""
    env: dict[str, str] = {}
    if not path.is_file():
        return env
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if "=" not in line:
            continue
        key, _, value = line.partition("=")
        # strip optional surrounding quotes
        value = value.strip().strip("\"'")
        env[key.strip()] = value
    return env


def _get_credentials() -> tuple[str, str]:
    """
    Return (smtp_user, smtp_password).

    Resolution order:
      1. SUSTECH_SMTP_USER / SUSTECH_SMTP_PASS env vars
      2. TAOXI_SMTP_USER / TAOXI_SMTP_PASS env vars (legacy)
      3. ~/.sustech-mailer-smtp.env file
      4. ~/.tao-ci-smtp.env file (legacy fallback)
    """
    user = os.environ.get("SUSTECH_SMTP_USER") or os.environ.get("TAOXI_SMTP_USER")
    password = os.environ.get("SUSTECH_SMTP_PASS") or os.environ.get("TAOXI_SMTP_PASS")

    if user and password:
        return user, password

    # Try credential files in order
    env: dict[str, str] = {}
    for env_path in ENV_FILES:
        env = _load_env_file(env_path)
        if env:
            break

    user = user or env.get("SUSTECH_SMTP_USER") or env.get("TAOXI_SMTP_USER", DEFAULT_SENDER)
    password = password or env.get("SUSTECH_SMTP_PASS") or env.get("TAOXI_SMTP_PASS", "")

    if not password:
        print(
            f"ERROR: SMTP password not found.\n"
            f"Set SUSTECH_SMTP_PASS env var or create ~/.sustech-mailer-smtp.env with:\n"
            f"  SUSTECH_SMTP_PASS=your_authorization_code\n",
            file=sys.stderr,
        )
        sys.exit(1)

    return user, password


def _attach_file(msg: MIMEMultipart, filepath: Path) -> None:
    """Attach a file to the MIME message with RFC 2231 encoded filename."""
    if not filepath.is_file():
        print(f"WARNING: attachment not found, skipping: {filepath}", file=sys.stderr)
        return

    with open(filepath, "rb") as f:
        part = MIMEBase("application", "octet-stream")
        part.set_payload(f.read())
    encoders.encode_base64(part)
    # Use RFC 2231 encoding for CJK and non-ASCII filenames
    encoded_name = encode_rfc2231(filepath.name, charset="utf-8")
    part.add_header(
        "Content-Disposition",
        "attachment",
        filename=encoded_name,
    )
    msg.attach(part)


# ── main ────────────────────────────────────────────────────────────────────


def build_message(
    *,
    to: str,
    subject: str,
    body: str,
    sender: str = DEFAULT_SENDER,
    cc: str | None = None,
    attachments: list[Path] | None = None,
) -> MIMEMultipart:
    """Build a MIME email message with optional CC and attachments."""
    msg = MIMEMultipart()
    msg["From"] = f"{SENDER_DISPLAY} <{sender}>"
    msg["To"] = to
    msg["Subject"] = subject
    if cc:
        msg["Cc"] = cc

    msg.attach(MIMEText(body, "plain", "utf-8"))

    if attachments:
        for path in attachments:
            _attach_file(msg, path)

    return msg


def send(
    *,
    to: str,
    subject: str,
    body: str,
    mode: str = "default",
    cc: str | None = None,
    attachments: list[Path] | None = None,
    dry_run: bool = False,
) -> bool:
    """
    Send an email via SMTP SSL.

    Args:
        to: Recipient email address.
        subject: Email subject line.
        body: Plain-text email body.
        mode: Email mode ('default' or 'homework').
        cc: Optional CC recipient(s), comma-separated.
        attachments: List of file paths to attach.
        dry_run: If True, connect and authenticate but do not send.

    Returns:
        True if the email was sent (or dry-run succeeded), False otherwise.
    """
    user, password = _get_credentials()
    msg = build_message(
        to=to, subject=subject, body=body, sender=user, cc=cc, attachments=attachments
    )

    # Use certifi CA bundle if available (fixes macOS SSL issues)
    try:
        import certifi
        context = ssl.create_default_context(cafile=certifi.where())
    except ImportError:
        context = ssl.create_default_context()

    try:
        with smtplib.SMTP_SSL(SMTP_SERVER, SMTP_PORT, context=context, timeout=30) as server:
            server.login(user, password)
            if dry_run:
                print(f"DRY-RUN OK: connected and authenticated as {user}")
                print(f"  To:      {to}")
                if cc:
                    print(f"  CC:      {cc}")
                print(f"  Subject: {subject}")
                print(f"  Mode:    {mode}")
                print("-" * 40)
                print(body)
                print("-" * 40)
                if attachments:
                    for a in attachments:
                        status = "✓" if a.is_file() else "✗ MISSING"
                        print(f"  Attach:  {a.name} [{status}]")
                else:
                    print(f"  Attach:  (none)")
                return True

            recipients = [to]
            if cc:
                recipients.extend(addr.strip() for addr in cc.split(","))
            server.sendmail(user, recipients, msg.as_string())
            print(f"✓ Email sent to {to} (mode: {mode})")
            return True

    except smtplib.SMTPAuthenticationError as e:
        print(f"ERROR: authentication failed — {e}", file=sys.stderr)
        print(
            "Hint: use an authorization code (授权码), not your login password.",
            file=sys.stderr,
        )
        return False
    except smtplib.SMTPException as e:
        print(f"ERROR: SMTP failure — {e}", file=sys.stderr)
        return False
    except OSError as e:
        print(f"ERROR: network error — {e}", file=sys.stderr)
        return False


def main() -> None:
    """CLI entry point."""
    parser = argparse.ArgumentParser(
        description="Send an email via SUSTech student mailbox SMTP.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("--to", required=True, help="Recipient email address")
    parser.add_argument("--subject", required=True, help="Email subject line")

    body_group = parser.add_mutually_exclusive_group(required=True)
    body_group.add_argument("--body", help="Email body text (inline)")
    body_group.add_argument("--body-file", type=Path, help="Read body from file")

    parser.add_argument(
        "--mode",
        choices=["default", "homework"],
        default="default",
        help="Email mode: 'default' (English, with CV/transcript) or 'homework' (Chinese, no CV)",
    )
    parser.add_argument(
        "--no-attachments",
        action="store_true",
        help="Do not attach any default files",
    )
    parser.add_argument(
        "--extra-attachment",
        type=Path,
        action="append",
        default=[],
        help="Additional file(s) to attach (repeatable)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Connect and authenticate but do not send",
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

    # build attachment list based on mode
    attachments: list[Path] = []
    if not args.no_attachments:
        if args.mode == "default":
            # Default mode: attach CV and transcript
            attachments.extend(DEFAULT_ATTACHMENTS)
        # Homework mode: no default attachments (only user-specified extras)

    attachments.extend(args.extra_attachment)

    # Warn if homework mode has no attachments (likely a mistake)
    if args.mode == "homework" and not attachments:
        print(
            "WARNING: homework mode with no attachments — did you forget --extra-attachment?",
            file=sys.stderr,
        )

    ok = send(
        to=args.to,
        subject=args.subject,
        body=body,
        mode=args.mode,
        cc=args.cc,
        attachments=attachments,
        dry_run=args.dry_run,
    )
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
