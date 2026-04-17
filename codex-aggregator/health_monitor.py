import requests
import time
import os

# Configuration
TOKENS = os.getenv("COPILOT_TOKENS", "").split(",")  # Comma-separated list of tokens
NOTIFICATION_URL = os.getenv("NOTIFICATION_URL")  # e.g., Bark or Telegram webhook
CHECK_INTERVAL = 3600  # Check every hour

def send_alert(message):
    if NOTIFICATION_URL:
        try:
            requests.get(f"{NOTIFICATION_URL}/{message}")
            print(f"Alert sent: {message}")
        except Exception as e:
            print(f"Failed to send alert: {e}")

def check_token(token):
    url = "https://api.github.com/user/billing/copilot"
    headers = {
        "Authorization": f"Bearer {token}",
        "Accept": "application/vnd.github+json",
    }
    try:
        response = requests.get(url, headers=headers)
        if response.status_code == 200:
            return True, "Active"
        elif response.status_code == 422:
            return False, "Subscription Expired or Billing Issue"
        elif response.status_code == 401:
            return False, "Token Invalid or Revoked"
        else:
            return False, f"Unexpected Status: {response.status_code}"
    except Exception as e:
        return False, f"Error: {str(e)}"

def main():
    print("Starting Copilot Health Monitor...")
    while True:
        for token in TOKENS:
            if not token: continue
            is_active, status = check_token(token)
            if not is_active:
                msg = f"Codex Alert: Account [{token[:8]}...] status: {status}"
                send_alert(msg)
        time.sleep(CHECK_INTERVAL)

if __name__ == "__main__":
    main()
