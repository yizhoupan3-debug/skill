import os
import base64
import time
import requests
import xml.etree.ElementTree as ET
from fastapi import FastAPI, Request, Response
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.backends import default_backend
import struct

# --- 配置区 (请填入你的真实凭证) ---
CORP_ID = os.getenv("WX_CORP_ID", "YOUR_CORP_ID")
AGENT_ID = os.getenv("WX_AGENT_ID", "YOUR_AGENT_ID")
SECRET = os.getenv("WX_SECRET", "YOUR_SECRET")
TOKEN = os.getenv("WX_TOKEN", "YOUR_TOKEN")
AES_KEY = os.getenv("WX_AES_KEY", "YOUR_AES_KEY")

# AI 配置 (示例使用 DeepSeek，兼容 OpenAI 格式)
AI_API_KEY = os.getenv("AI_API_KEY", "YOUR_AI_API_KEY")
AI_BASE_URL = "https://api.deepseek.com/v1"

app = FastAPI()

class WXBizMsgCrypt:
    def __init__(self, token, aes_key, corp_id):
        self.token = token
        self.aes_key = base64.b64decode(aes_key + "=")
        self.corp_id = corp_id

    def decrypt(self, encrypt_msg):
        cipher = Cipher(algorithms.AES(self.aes_key), modes.CBC(self.aes_key[:16]), backend=default_backend())
        decryptor = cipher.decryptor()
        plain_text = decryptor.update(base64.b64decode(encrypt_msg)) + decryptor.finalize()
        pad = plain_text[-1]
        content = plain_text[16:-pad]
        xml_len = struct.unpack(">I", content[:4])[0]
        return content[4:xml_len+4].decode('utf-8')

def get_access_token():
    url = f"https://qyapi.weixin.qq.com/cgi-bin/gettoken?corpid={CORP_ID}&corpsecret={SECRET}"
    return requests.get(url).json().get("access_token")

def send_to_user(user_id, content):
    token = get_access_token()
    url = f"https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={token}"
    data = {
        "touser": user_id,
        "msgtype": "text",
        "agentid": AGENT_ID,
        "text": {"content": content},
        "safe": 0
    }
    requests.post(url, json=data)

def ask_ai(prompt):
    headers = {"Authorization": f"Bearer {AI_API_KEY}", "Content-Type": "application/json"}
    payload = {
        "model": "deepseek-chat",
        "messages": [{"role": "user", "content": prompt}]
    }
    try:
        res = requests.post(f"{AI_BASE_URL}/chat/completions", json=payload, headers=headers)
        return res.json()['choices'][0]['message']['content']
    except Exception as e:
        return f"AI 脑壳痛了: {e}"

@app.get("/webhook")
async def verify(msg_signature: str, timestamp: str, nonce: str, echostr: str):
    crypt = WXBizMsgCrypt(TOKEN, AES_KEY, CORP_ID)
    return Response(content=crypt.decrypt(echostr))

@app.post("/webhook")
async def handle_msg(request: Request):
    body = await request.body()
    xml_data = ET.fromstring(body)
    encrypt_msg = xml_data.find("Encrypt").text
    
    # 解密消息
    crypt = WXBizMsgCrypt(TOKEN, AES_KEY, CORP_ID)
    xml_content = crypt.decrypt(encrypt_msg)
    msg_xml = ET.fromstring(xml_content)
    
    user_id = msg_xml.find("FromUserName").text
    content = msg_xml.find("Content").text
    
    # 异步调用 AI 并回复 (简单实现)
    ai_reply = ask_ai(content)
    send_to_user(user_id, ai_reply)
    
    return Response(content="success")

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
