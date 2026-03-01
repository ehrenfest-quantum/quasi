-- 003_telemetry.sql — reasoning & PR-outcome gaps
-- Safe to run multiple times.

-- Persist the gate/reviewer reasoning text so domain-reasoning quality
-- is measurable without parsing stdout logs.
ALTER TABLE senate_telemetry
    ADD COLUMN IF NOT EXISTS verdict_reasoning TEXT,
    ADD COLUMN IF NOT EXISTS verdict_issues     TEXT;  -- JSON array, B2 only

-- Index for quality-analysis queries that filter on verdict + reasoning presence.
CREATE INDEX IF NOT EXISTS idx_telemetry_verdict_reasoning
    ON senate_telemetry (role, downstream_verdict)
    WHERE verdict_reasoning IS NOT NULL;
