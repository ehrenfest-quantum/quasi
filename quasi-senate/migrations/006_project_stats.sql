-- Project-level statistics: LOC per component, roadmap progress.
-- Used by collect_project_stats.py; table created by quasi-senate at startup.

CREATE TABLE IF NOT EXISTS project_stats (
    id          BIGSERIAL PRIMARY KEY,
    snapshot_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    component   TEXT NOT NULL,
    category    TEXT NOT NULL,
    language    TEXT NOT NULL,
    loc_source  INTEGER NOT NULL,
    loc_tests   INTEGER NOT NULL,
    loc_docs    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS roadmap_progress (
    id          BIGSERIAL PRIMARY KEY,
    snapshot_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    phase       INTEGER NOT NULL,
    phase_name  TEXT NOT NULL,
    theme       TEXT NOT NULL,
    pct         INTEGER NOT NULL,
    done        TEXT,
    missing     TEXT
);

CREATE INDEX IF NOT EXISTS idx_project_stats_snapshot ON project_stats (snapshot_at);
CREATE INDEX IF NOT EXISTS idx_roadmap_progress_snapshot ON roadmap_progress (snapshot_at);
