FROM python:3.9-slim
WORKDIR /app
COPY server.py .
COPY static static/
RUN pip install flask mysql-connector-python requests
EXPOSE 5000
CMD ["python", "server.py"]
