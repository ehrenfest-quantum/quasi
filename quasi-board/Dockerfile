FROM python:3.12-slim

WORKDIR /app

# Create directories the server expects
RUN mkdir -p /home/vops/quasi-ledger /home/vops/quasi-board/keys

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY . .

EXPOSE 8420

CMD ["uvicorn", "server:app", "--host", "0.0.0.0", "--port", "8420"]
