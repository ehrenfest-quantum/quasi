#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 QUASI Contributors
"""HAL Contract client bindings for IBM/IQM backends.

Implements the HAL Contract specification for job submission, status polling,
and result retrieval from QUASI-compatible quantum hardware providers.
"""

import json
from typing import Any
import urllib.request
import urllib.error


# HAL Contract endpoints for supported backends
BACKEND_ENDPOINTS = {
    "ibm_torino": "https://api.ibm.com/hal/v2/jobs",
    "iqm_garnet": "https://api.iqm.fi/hal/v2/jobs",
}


def submit_job(qasm_str: str, backend: str) -> dict[str, Any]:
    """Submit a QASM3 job to a HAL Contract-compatible backend.

    Args:
        qasm_str: OpenQASM 3 program as a string.
        backend: Backend identifier (e.g., "ibm_torino", "iqm_garnet").

    Returns:
        dict: Response containing job_id and initial status.

    Raises:
        ValueError: If backend is not supported.
        urllib.error.URLError: If the HTTP request fails.
    """
    if backend not in BACKEND_ENDPOINTS:
        raise ValueError(
            f"Unsupported backend: {backend}. "
            f"Supported: {list(BACKEND_ENDPOINTS.keys())}"
        )

    url = BACKEND_ENDPOINTS[backend]
    payload = {
        "qasm": qasm_str,
        "backend": backend,
        "shots": 1000,  # Default shots per HAL Contract spec
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
            return response
    except urllib.error.HTTPError as e:
        error_body = e.read().decode()
        raise urllib.error.HTTPError(
            url, e.code, error_body, e.headers, e.fp
        ) from e


def get_job_status(job_id: str, backend: str) -> dict[str, Any]:
    """Query the status of a submitted job.

    Args:
        job_id: Job identifier returned by submit_job.
        backend: Backend identifier (e.g., "ibm_torino", "iqm_garnet").

    Returns:
        dict: Job status information including 'status' field and optional results.

    Raises:
        ValueError: If backend is not supported.
        urllib.error.URLError: If the HTTP request fails.
    """
    if backend not in BACKEND_ENDPOINTS:
        raise ValueError(
            f"Unsupported backend: {backend}. "
            f"Supported: {list(BACKEND_ENDPOINTS.keys())}"
        )

    base_url = BACKEND_ENDPOINTS[backend]
    url = f"{base_url}/{job_id}"

    req = urllib.request.Request(
        url,
        headers={
            "Accept": "application/json",
            "User-Agent": "quasi-agent/0.1",
        },
        method="GET",
    )

    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            response = json.loads(resp.read())
            return response
    except urllib.error.HTTPError as e:
        error_body = e.read().decode()
        raise urllib.error.HTTPError(
            url, e.code, error_body, e.headers, e.fp
        ) from e
