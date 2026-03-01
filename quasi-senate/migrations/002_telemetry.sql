-- 002_telemetry.sql — telemetry schema v2
-- Adds pipeline_attempt column and pr_outcomes table.
-- Safe to run multiple times (IF NOT EXISTS / IF NOT EXIST guards).

ALTER TABLE senate_telemetry
    ADD COLUMN IF NOT EXISTS pipeline_attempt INTEGER NOT NULL DEFAULT 1;

-- pr_outcomes — one row per senate-opened PR, updated by the CI poller.
CREATE TABLE IF NOT EXISTS pr_outcomes (
    id              BIGSERIAL PRIMARY KEY,
    pr_url          TEXT NOT NULL UNIQUE,
    pr_number       INTEGER,
    issue_number    INTEGER,
    b1_solver_model TEXT,
    b1_cycle_id     UUID,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- 'pending' | 'passing' | 'failing' | 'error'
    ci_status       TEXT CHECK (ci_status IN ('pending', 'passing', 'failing', 'error'))
                    NOT NULL DEFAULT 'pending',
    ci_checked_at   TIMESTAMPTZ,
    merged          BOOLEAN,
    merged_at       TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_pr_outcomes_issue     ON pr_outcomes (issue_number);
CREATE INDEX IF NOT EXISTS idx_pr_outcomes_ci_status ON pr_outcomes (ci_status, ci_checked_at);
CREATE INDEX IF NOT EXISTS idx_pr_outcomes_cycle     ON pr_outcomes (b1_cycle_id);
