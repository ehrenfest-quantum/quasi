-- Paula Scale: Q-Level computation from roadmap_progress.
--
-- The Paula Scale measures quantum computational capability on a 0–3 scale:
--   Q-Level 0: No quantum computation (classical only)
--   Q-Level 1: Molecular-scale quantum simulation (MVP: compile + execute + verify)
--   Q-Level 2: Planetary-scale quantum simulation
--   Q-Level 3: Universal quantum simulation
--
-- QUASI's roadmap maps to the first Q-Level (0 → 1):
--   L1 (Language):   Phases 1-3   → Q 0.00–0.20
--   L2 (Compiler):   Phases 4-7   → Q 0.20–0.60
--   L3 (Hardware):   Phases 8-10  → Q 0.60–0.85
--   L4 (Runtime):    Phases 11-15 → Q 0.85–1.00

CREATE TABLE IF NOT EXISTS paula_scale (
    id          BIGSERIAL PRIMARY KEY,
    snapshot_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    q_level     NUMERIC(5,4) NOT NULL,
    l1_pct      NUMERIC(5,2) NOT NULL,
    l2_pct      NUMERIC(5,2) NOT NULL,
    l3_pct      NUMERIC(5,2) NOT NULL,
    l4_pct      NUMERIC(5,2) NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_paula_scale_snapshot ON paula_scale (snapshot_at);
