FROM python:3.12-slim

RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*
RUN useradd -m -u 1000 quasi

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY --chown=quasi:quasi . .

USER quasi

ENV QUASI_DOMAIN=localhost \
    QUASI_DATA_DIR=/data/board \
    QUASI_LEDGER_DIR=/data/ledger

EXPOSE 8420

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD curl -f http://localhost:8420/quasi-board/health || exit 1

CMD ["uvicorn", "server:app", "--host", "0.0.0.0", "--port", "8420"]
