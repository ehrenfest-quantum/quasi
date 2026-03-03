#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 QUASI Contributors
"""Unit tests for HAL Contract client bindings.

Validates that HTTP request bodies conform to the HAL Contract JSON schema.
"""

import json
from unittest.mock import Mock, patch

import pytest

from hal_client import submit_job, get_job_status, BACKEND_ENDPOINTS


def test_submit_job_payload_schema():
    """Validate that submit_job sends a HAL Contract-compliant JSON payload."""
    qasm_str = """OPENQASM 3;
include "stdgates.inc";
qubit[2] q;
h q[0];
cx q[0], q[1];
"""
    backend = "ibm_torino"

    mock_response = {"job_id": "test-job-123", "status": "queued"}

    with patch("urllib.request.urlopen") as mock_urlopen:
        mock_resp = Mock()
        mock_resp.read.return_value = json.dumps(mock_response).encode()
        mock_urlopen.return_value.__enter__.return_value = mock_resp

        result = submit_job(qasm_str, backend)

        # Verify the request was made
        assert mock_urlopen.called
        req = mock_urlopen.call_args[0][0]

        # Validate HTTP method
        assert req.method == "POST"

        # Validate headers
        assert req.headers["Content-Type"] == "application/json"
        assert req.headers["Accept"] == "application/json"

        # Validate payload structure
        payload = json.loads(req.data)
        assert "qasm" in payload
        assert "backend" in payload
        assert "shots" in payload
        assert payload["qasm"] == qasm_str
        assert payload["backend"] == backend
        assert isinstance(payload["shots"], int)
        assert payload["shots"] > 0

        # Validate response
        assert result["job_id"] == "test-job-123"
        assert result["status"] == "queued"


def test_submit_job_unsupported_backend():
    """Test that submit_job raises ValueError for unsupported backends."""
    qasm_str = "OPENQASM 3; qubit[1] q; h q[0];"
    backend = "unsupported_backend"

    with pytest.raises(ValueError, match="Unsupported backend"):
        submit_job(qasm_str, backend)


def test_get_job_status_request():
    """Validate that get_job_status sends a properly formatted GET request."""
    job_id = "test-job-123"
    backend = "iqm_garnet"

    mock_response = {"job_id": job_id, "status": "completed", "results": {}}

    with patch("urllib.request.urlopen") as mock_urlopen:
        mock_resp = Mock()
        mock_resp.read.return_value = json.dumps(mock_response).encode()
        mock_urlopen.return_value.__enter__.return_value = mock_resp

        result = get_job_status(job_id, backend)

        # Verify the request was made
        assert mock_urlopen.called
        req = mock_urlopen.call_args[0][0]

        # Validate HTTP method
        assert req.method == "GET"

        # Validate URL includes job_id
        assert job_id in req.full_url

        # Validate headers
        assert req.headers["Accept"] == "application/json"

        # Validate response
        assert result["job_id"] == job_id
        assert result["status"] == "completed"


def test_get_job_status_unsupported_backend():
    """Test that get_job_status raises ValueError for unsupported backends."""
    job_id = "test-job-123"
    backend = "unsupported_backend"

    with pytest.raises(ValueError, match="Unsupported backend"):
        get_job_status(job_id, backend)


def test_backend_endpoints_configured():
    """Verify that all required backends have endpoints configured."""
    assert "ibm_torino" in BACKEND_ENDPOINTS
    assert "iqm_garnet" in BACKEND_ENDPOINTS
    assert all(url.startswith("https://") for url in BACKEND_ENDPOINTS.values())
