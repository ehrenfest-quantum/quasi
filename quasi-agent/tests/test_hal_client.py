import pytest
from unittest import mock

from quasi_agent.hal_client import submit_job, get_job_status


def test_submit_job_payload():
    with mock.patch('quasi_agent.hal_client.requests.post') as mock_post:
        mock_resp = mock.Mock()
        mock_resp.json.return_value = {"job_id": "123"}
        mock_resp.raise_for_status = mock.Mock()
        mock_post.return_value = mock_resp

        qasm = "OPENQASM 3.0; // dummy circuit"
        backend = "ibm_torino"
        job_id = submit_job(qasm, backend)

        # Verify the request payload matches the expected schema
        _, kwargs = mock_post.call_args
        payload = kwargs.get('json')
        assert isinstance(payload, dict)
        assert payload["qasm"] == qasm
        assert payload["backend"] == backend
        assert "shots" in payload
        assert job_id == "123"


def test_get_job_status():
    with mock.patch('quasi_agent.hal_client.requests.get') as mock_get:
        mock_resp = mock.Mock()
        mock_resp.json.return_value = {"status": "COMPLETED"}
        mock_resp.raise_for_status = mock.Mock()
        mock_get.return_value = mock_resp

        status = get_job_status("123")
        mock_get.assert_called_once_with("http://example.com/status/123")
        assert status == "COMPLETED"
