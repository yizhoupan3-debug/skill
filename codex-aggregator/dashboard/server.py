from flask import Flask, jsonify
import mysql.connector
import os
from datetime import datetime, timedelta

app = Flask(__name__)

# Config from Env
DB_HOST = os.getenv("DB_HOST", "db")
DB_USER = os.getenv("DB_USER", "root")
DB_PASS = os.getenv("DB_PASS", "123456")
DB_NAME = os.getenv("DB_NAME", "oneapi")

def get_db_connection():
    return mysql.connector.connect(
        host=DB_HOST,
        user=DB_USER,
        password=DB_PASS,
        database=DB_NAME
    )

@app.route('/api/quotas')
def get_quotas():
    try:
        conn = get_db_connection()
        cursor = conn.cursor(dictionary=True)
        
        # 1. Get all active channels (accounts)
        cursor.execute("SELECT id, name, type, status, channel_id FROM channels WHERE type = 15") # 15 is typical for GitHub Copilot in one-api/new-api
        channels = cursor.fetchall()
        
        results = []
        for ch in channels:
            # 5h limit
            cursor.execute("""
                SELECT count(*) as count FROM logs 
                WHERE channel_id = %s AND created_at > UNIX_TIMESTAMP(NOW() - INTERVAL 5 HOUR)
            """, (ch['id'],))
            count_5h = cursor.fetchone()['count']
            
            # 7d limit
            cursor.execute("""
                SELECT count(*) as count FROM logs 
                WHERE channel_id = %s AND created_at > UNIX_TIMESTAMP(NOW() - INTERVAL 7 DAY)
            """, (ch['id'],))
            count_7d = cursor.fetchone()['count']
            
            results.append({
                "id": ch['id'],
                "name": ch['name'],
                "status": "active" if ch['status'] == 1 else "red",
                "used_5h": count_5h,
                "used_7d": count_7d
            })
            
        cursor.close()
        conn.close()
        return jsonify(results)
    except Exception as e:
        return jsonify({"error": str(e)}), 500

@app.route('/')
def index():
    return app.send_static_file('index.html')

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=5000)
