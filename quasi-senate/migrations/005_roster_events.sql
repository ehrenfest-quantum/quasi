-- Roster lifecycle events (discovery, quarantine, profiling, delisting)
-- Used by quasi-roster Python tool; table created by quasi-senate at startup.

CREATE TABLE IF NOT EXISTS roster_events (
    id          BIGSERIAL PRIMARY KEY,
    timestamp   TIMESTAMPTZ NOT NULL DEFAULT now(),
    event_type  TEXT NOT NULL,  -- discovered, quarantined, restored, profiled, delisted
    model_id    TEXT NOT NULL,
    provider    TEXT NOT NULL,
    details     JSONB,
    applied     BOOLEAN NOT NULL DEFAULT false
);

CREATE INDEX IF NOT EXISTS idx_roster_events_timestamp ON roster_events (timestamp);
CREATE INDEX IF NOT EXISTS idx_roster_events_model ON roster_events (model_id);
