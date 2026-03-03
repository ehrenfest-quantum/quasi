#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 QUASI Contributors
"""
HAL Contract client bindings for IBM/IQM backends.

Implements the HAL Contract specification for job submission,
status polling, and result retrieval from QUASI-compatible hardware.
"""

import json
import os
from typing import Any, Literal
import urllib.request
import urllib.error


# HAL Contract v2.2 job states
JobStatus = Literal[
    "queued",
    "running",
    "completed",
    "failed",
    "cancelled",
]


# Backend identifiers
Backend = Literal[
    "ibm_torino",
    "iqm_garnet",
]


# Default HAL endpoints (can be overridden via environment)
DEFAULT_ENDPOINTS: dict[Backend, str] = {
    "ibm_torino": "https://hal.ibm.example.com/v1",
    "iqm_garnet": "https://hal.iqm.example.com/v1",
}


def _get_endpoint(backend: Backend) -> str:
    """Get the HAL endpoint URL for a backend."""
    env_key = f"HAL_{backend.upper()}_ENDPOINT"
    return os.environ.get(env_key, DEFAULT_ENDPOINTS[backend])


def submit_job(qasm_str: str, backend: Backend) -> str:
    """Submit a QASM3 job to a HAL-compatible backend.

    Args:
        qasm_str: OpenQASM 3 program as a string.
        backend: Backend identifier (e.g., "ibm_torino", "iqm_garnet").

    Returns:
        Job ID string.

    Raises:
        ValueError: If qasm_str is empty or backend is invalid.
        RuntimeError: If the submission fails.
    """
    if not qasm_str or not qasm_str.strip():
        raise ValueError("qasm_str must be a non-empty string")

    if backend not in DEFAULT_ENDPOINTS:
        raise ValueError(f"Unknown backend: {backend}")

    endpoint = _get_endpoint(backend)
    url = f"{endpoint}/jobs"

    # HAL Contract v2.2 job submission schema
    payload = {
        "qasm": qasm_str,
        "backend": backend,
        "format": "qasm3",
    }

    data = json.dumps(payload).encode()
    req = urllib.request.Request(
        url,
        data=data,
        headers={
            "Content-Type": "application/json",
            "Accept": "application/json",
            "User-Agent": "quasi-agent/0.1",
        },
        method="POST",
    )

    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            response = json.loads(resp.read())
            job_id = response.get("job_id")
            if not job_id:
                raise RuntimeError("Response missing job_id field")
            return job_id
    except urllib.error.HTTPError as e:
        error_body = e.read().decode()
        raise RuntimeError(f"HTTP {e.code}: {error_body}") from e
    except Exception as e:
        raise RuntimeError(f"Submission failed: {e}") from e


def get_job_status(job_id: str) -> dict[str, Any]:
    """Query the status of a submitted job.

    Args:
        job_id: Job ID returned by submit_job.

    Returns:
        Dictionary with at least:
            - "status": JobStatus string
            - "backend": Backend identifier
            - "result": Optional result data (if completed)

    Raises:
        ValueError: If job_id is empty.
        RuntimeError: If the query fails.
    """
    if not job_id or not job_id.strip():
        raise ValueError("job_id must be a non-empty string")

    # For now, we need to determine which backend this job belongs to.
    # In a real implementation, this would be stored or queried from a registry.
    # We'll try all known backends.
    for backend in DEFAULT_ENDPOINTS:
        endpoint = _get_endpoint(backend)
        url = f"{endpoint}/jobs/{job_id}"

        req = urllib.request.Request(
            url,
            headers={
                "Accept": "application/json",
                "User-Agent": "quasi-agent/0.1",
            },
        )

        try:
            with urllib.request.urlopen(req, timeout=10) as resp:
                response = json.loads(resp.read())
                # Validate response has required fields
                if "status" not in response:
                    raise RuntimeError("Response missing status field")
                return response
        except urllib.error.HTTPError as e:
            if e.code == 404:
                continue  # Try next backend
            raise RuntimeError(f"HTTP {e.code}: {e.read().decode()}") from e
        except Exception as e:
            raise RuntimeError(f"Query failed: {e}") from e

    raise RuntimeError(f"Job {job_id} not found on any known backend")
