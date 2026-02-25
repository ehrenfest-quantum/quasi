FROM python:3.11-slim

WORKDIR /app

COPY quasi-board/requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY quasi-board/server.py .
COPY quasi-board/spec ./spec

EXPOSE 8420

CMD ["python", "server.py"]