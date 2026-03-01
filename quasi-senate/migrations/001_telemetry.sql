CREATE TABLE IF NOT EXISTS senate_telemetry (
    id              BIGSERIAL PRIMARY KEY,
    timestamp       TIMESTAMPTZ NOT NULL DEFAULT now(),
    cycle_id        UUID NOT NULL,
    role            TEXT NOT NULL CHECK (role IN ('A1_council', 'A2_drafter', 'A3_gate', 'B1_solver', 'B2_reviewer')),
    model_id        TEXT NOT NULL,
    model_string    TEXT NOT NULL,
    provider        TEXT NOT NULL,
    base_model      TEXT NOT NULL,
    level           SMALLINT,
    issue_number    INTEGER,
    latency_ms      BIGINT NOT NULL,
    input_tokens_approx  BIGINT,
    output_tokens_approx BIGINT,
    http_status     SMALLINT,
    retries         INTEGER NOT NULL DEFAULT 0,
    json_parse_ok   BOOLEAN,
    downstream_verdict TEXT,
    model_verified  BOOLEAN,
    served_model    TEXT,
    error           TEXT,
    dry_run         BOOLEAN NOT NULL DEFAULT false
);

CREATE INDEX IF NOT EXISTS idx_telemetry_timestamp ON senate_telemetry (timestamp);
CREATE INDEX IF NOT EXISTS idx_telemetry_base_model_provider ON senate_telemetry (base_model, provider);
CREATE INDEX IF NOT EXISTS idx_telemetry_role ON senate_telemetry (role);
CREATE INDEX IF NOT EXISTS idx_telemetry_cycle_id ON senate_telemetry (cycle_id);
CREATE INDEX IF NOT EXISTS idx_telemetry_provider ON senate_telemetry (provider);
