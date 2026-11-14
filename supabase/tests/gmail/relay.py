"""Forward the two captured GoTrue MIME messages using a transient credential."""
import base64
import email
import email.policy
import json
import smtplib
import ssl
import sys

payload = json.load(sys.stdin)
raw = base64.b64decode(payload["mime"], validate=True)
if len(raw) > 65536:
    sys.exit(2)
msg = email.message_from_bytes(raw, policy=email.policy.default)
if [a.addr_spec for a in msg["To"].addresses] != [payload["recipient"]]:
    sys.exit(2)
if msg["From"].addresses[0].addr_spec != "onboarding@resend.dev":
    sys.exit(2)
if msg["Subject"] != payload["subject"]:
    sys.exit(2)
if payload.get("offline") is True:
    sys.exit(0)
try:
    with smtplib.SMTP("smtp.resend.com", 587, timeout=20) as smtp:
        smtp.starttls(context=ssl.create_default_context())
        smtp.login("resend", payload.pop("key").strip())
        failures = smtp.sendmail("onboarding@resend.dev", [payload["recipient"]], raw)
        if failures:
            sys.exit(3)
except (OSError, smtplib.SMTPException):
    sys.exit(3)
