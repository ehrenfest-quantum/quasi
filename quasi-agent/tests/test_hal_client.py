#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright 2026 QUASI Contributors
"""
Unit tests for HAL Contract client bindings.

Validates that HTTP request bodies conform to the HAL Contract JSON schema.
"""

import json
from unittest.mock import Mock, patch
import urllib.error

import pytest

import sys
from pathlib import Path

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from hal_client import Backend, JobStatus, submit_job, get_job_status


# Sample QASM3 program
SAMPLE_QASM = """
OPENQASM 3;
include "stdgates.inc";
qubit[2] q;
bit[2] c;
h q[0];
cx q[0], q[1];
c[0] = measure q[0];
c[1] = measure q[1];
"""


class TestSubmitJob:
    """Tests for submit_job function."""

    def test_submit_job_validates_qasm_str(self):
        """submit_job raises ValueError for empty qasm_str."""
        with pytest.raises(ValueError, match="qasm_str must be a non-empty string"):
            submit_job("", "ibm_torino")

        with pytest.raises(ValueError, match="qasm_str must be a non-empty string"):
            submit_job("   ", "ibm_torino")

    def test_submit_job_validates_backend(self):
        """submit_job raises ValueError for unknown backend."""
        with pytest.raises(ValueError, match="Unknown backend"):
            submit_job(SAMPLE_QASM, "unknown_backend")  # type: ignore

    @patch("hal_client.urllib.request.urlopen")
    def test_submit_job_sends_correct_payload(self, mock_urlopen):
        """submit_job sends HAL Contract v2.2 compliant JSON payload."""
        # Mock successful response
        mock_response = Mock()
        mock_response.read.return_value = json.dumps({"job_id": "test-job-123"}).encode()
        mock_urlopen.return_value.__enter__.return_value = mock_response

        job_id = submit_job(SAMPLE_QASM, "ibm_torino")

        # Verify request was made
        assert mock_urlopen.called
        req = mock_urlopen.call_args[0][0]

        # Verify HTTP method and headers
        assert req.method == "POST"
        assert req.headers["Content-Type"] == "application/json"
        assert req.headers["Accept"] == "application/json"

        # Verify payload structure matches HAL Contract schema
        payload = json.loads(req.data)
        assert payload["qasm"] == SAMPLE_QASM
        assert payload["backend"] == "ibm_torino"
        assert payload["format"] == "qasm3"

        # Verify job ID returned
        assert job_id == "test-job-123"

    @patch("hal_client.urllib.request.urlopen")
    def test_submit_job_iqm_backend(self, mock_urlopen):
        """submit_job works with IQM Garnet backend."""
        mock_response = Mock()
        mock_response.read.return_value = json.dumps({"job_id": "iqm-job-456"}).encode()
        mock_urlopen.return_value.__enter__.return_value = mock_response

        job_id = submit_job(SAMPLE_QASM, "iqm_garnet")

        req = mock_urlopen.call_args[0][0]
        payload = json.loads(req.data)
        assert payload["backend"] == "iqm_garnet"
        assert job_id == "iqm-job-456"

    @patch("hal_client.urllib.request.urlopen")
    def test_submit_job_http_error(self, mock_urlopen):
        """submit_job raises RuntimeError on HTTP error."""
        mock_urlopen.side_effect = urllib.error.HTTPError(
            url="http://test",
            code=500,
            msg="Internal Server Error",
            hdrs={},
            fp=None,
        )

        with pytest.raises(RuntimeError, match="HTTP 500"):
            submit_job(SAMPLE_QASM, "ibm_torino")

    @patch("hal_client.urllib.request.urlopen")
    def test_submit_job_missing_job_id(self, mock_urlopen):
        """submit_job raises RuntimeError when response lacks job_id."""
        mock_response = Mock()
        mock_response.read.return_value = json.dumps({}).encode()
        mock_urlopen.return_value.__enter__.return_value = mock_response

        with pytest.raises(RuntimeError, match="Response missing job_id field"):
            submit_job(SAMPLE_QASM, "ibm_torino")


class TestGetJobStatus:
    """Tests for get_job_status function."""

    def test_get_job_status_validates_job_id(self):
        """get_job_status raises ValueError for empty job_id."""
        with pytest.raises(ValueError, match="job_id must be a non-empty string"):
            get_job_status("")

        with pytest.raises(ValueError, match="job_id must be a non-empty string"):
            get_job_status("   ")

    @patch("hal_client.urllib.request.urlopen")
    def test_get_job_status_completed(self, mock_urlopen):
        """get_job_status returns completed job status."""
        mock_response = Mock()
        mock_response.read.return_value = json.dumps({
            "status": "completed",
            "backend": "ibm_torino",
            "result": {"counts": {"00": 500, "11": 500}},
        }).encode()
        mock_urlopen.return_value.__enter__.return_value = mock_response

        status = get_job_status("test-job-123")

        assert status["status"] == "completed"
        assert status["backend"] == "ibm_torino"
        assert "result" in status
        assert status["result"]["counts"]["00"] == 500

    @patch("hal_client.urllib.request.urlopen")
    def test_get_job_status_running(self, mock_urlopen):
        """get_job_status returns running job status."""
        mock_response = Mock()
        mock_response.read.return_value = json.dumps({
            "status": "running",
            "backend": "iqm_garnet",
        }).encode()
        mock_urlopen.return_value.__enter__.return_value = mock_response

        status = get_job_status("test-job-456")

        assert status["status"] == "running"
        assert status["backend"] == "iqm_garnet"

    @patch("hal_client.urllib.request.urlopen")
    def test_get_job_status_failed(self, mock_urlopen):
        """get_job_status returns failed job status."""
        mock_response = Mock()
        mock_response.read.return_value = json.dumps({
            "status": "failed",
            "backend": "ibm_torino",
            "error": "QPU calibration error",
        }).encode()
        mock_urlopen.return_value.__enter__.return_value = mock_response

        status = get_job_status("test-job-789")

        assert status["status"] == "failed"
        assert status["error"] == "QPU calibration error"

    @patch("hal_client.urllib.request.urlopen")
    def test_get_job_status_not_found(self, mock_urlopen):
        """get_job_status raises RuntimeError when job not found."""
        mock_urlopen.side_effect = urllib.error.HTTPError(
            url="http://test",
            code=404,
            msg="Not Found",
            hdrs={},
            fp=None,
        )

        with pytest.raises(RuntimeError, match="Job test-job-999 not found"):
            get_job_status("test-job-999")

    @patch("hal_client.urllib.request.urlopen")
    def test_get_job_status_missing_status_field(self, mock_urlopen):
        """get_job_status raises RuntimeError when response lacks status."""
        mock_response = Mock()
        mock_response.read.return_value = json.dumps({}).encode()
        mock_urlopen.return_value.__enter__.return_value = mock_response

        with pytest.raises(RuntimeError, match="Response missing status field"):
            get_job_status("test-job-123")
