import pytest
import httpx
from unittest.mock import patch, MagicMock

from hal_client import submit_job, get_job_status


def test_submit_job():
    # Mock the HTTP POST request
    mock_response = MagicMock()
    mock_response.json.return_value = {"job_id": "test-job-123", "status": "submitted"}
    mock_response.status_code = 201
    
    with patch('httpx.post', return_value=mock_response):
        result = submit_job("OPENQASM 3.0; qubit[2] q; h q[0]; cx q[0], q[1]; measure q;", "ibm_torino")
        
    assert result["job_id"] == "test-job-123"
    assert result["status"] == "submitted"


def test_get_job_status():
    # Mock the HTTP GET request
    mock_response = MagicMock()
    mock_response.json.return_value = {"job_id": "test-job-123", "status": "completed", "result": {"counts": {"00": 45, "11": 55}}}
    mock_response.status_code = 200
    
    with patch('httpx.get', return_value=mock_response):
        result = get_job_status("test-job-123")
        
    assert result["job_id"] == "test-job-123"
    assert result["status"] == "completed"
    assert result["result"]["counts"] == {"00": 45, "11": 55}


def test_submit_job_with_invalid_backend():
    # Test that invalid backend raises appropriate error
    mock_response = MagicMock()
    mock_response.status_code = 400
    mock_response.raise_for_status.side_effect = httpx.HTTPStatusError("Bad Request", request=MagicMock(), response=mock_response)
    
    with patch('httpx.post', return_value=mock_response):
        with pytest.raises(httpx.HTTPStatusError):
            submit_job("OPENQASM 3.0; qubit[2] q; h q[0]; cx q[0], q[1]; measure q;", "invalid_backend")