"""Postgres helpers for roster events and health queries."""

import json
import os
from contextlib import contextmanager

import psycopg2
import psycopg2.extras


def get_connection():
    """Connect using DATABASE_URL env var."""
    url = os.environ.get("DATABASE_URL")
    if not url:
        return None
    return psycopg2.connect(url)


@contextmanager
def db_cursor():
    """Context manager yielding a cursor that auto-commits."""
    conn = get_connection()
    if conn is None:
        yield None
        return
    try:
        with conn:
            with conn.cursor(cursor_factory=psycopg2.extras.RealDictCursor) as cur:
                yield cur
    finally:
        conn.close()


def record_event(cur, event_type, model_id, provider, details=None, applied=False):
    """Insert a roster_events row."""
    if cur is None:
        return
    cur.execute(
        """INSERT INTO roster_events (event_type, model_id, provider, details, applied)
           VALUES (%s, %s, %s, %s, %s)""",
        (event_type, model_id, provider, json.dumps(details) if details else None, applied),
    )


def query_health_failures(cur, threshold=3, days=7):
    """Return models with consecutive failures and zero successes in the window."""
    if cur is None:
        return []
    cur.execute(
        """SELECT model_id, provider,
                  COUNT(*) FILTER (WHERE status = 'FAIL') AS fails,
                  COUNT(*) FILTER (WHERE status = 'ok') AS oks,
                  COUNT(*) AS total
           FROM model_health
           WHERE checked_at > now() - interval '%s days'
           GROUP BY model_id, provider""",
        (days,),
    )
    rows = cur.fetchall()
    candidates = []
    for row in rows:
        if row["total"] < 5:
            continue  # cold-start guard
        if row["fails"] >= threshold and row["oks"] == 0:
            candidates.append(dict(row))
    return candidates


def query_telemetry_errors(cur, days=7):
    """Return models with high error/parse-failure rates from senate_telemetry."""
    if cur is None:
        return []
    cur.execute(
        """SELECT model_id, provider,
                  COUNT(*) AS total,
                  COUNT(*) FILTER (WHERE http_status >= 400 OR error IS NOT NULL) AS errors,
                  COUNT(*) FILTER (WHERE json_parse_ok = false) AS parse_fails
           FROM senate_telemetry
           WHERE timestamp > now() - interval '%s days'
           GROUP BY model_id, provider""",
        (days,),
    )
    return [dict(r) for r in cur.fetchall()]
