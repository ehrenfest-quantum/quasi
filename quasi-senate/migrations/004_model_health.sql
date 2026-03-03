CREATE TABLE IF NOT EXISTS model_health (
    id           BIGSERIAL PRIMARY KEY,
    checked_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    model_id     TEXT NOT NULL,
    provider     TEXT NOT NULL,
    status       TEXT NOT NULL,
    latency_ms   BIGINT,
    error        TEXT
);
CREATE INDEX IF NOT EXISTS idx_model_health_checked_at ON model_health (checked_at);
