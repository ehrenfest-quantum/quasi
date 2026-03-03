import pytest
import httpx
from unittest.mock import patch, MagicMock

# Import the functions we're testing
from hal_client import submit_job, get_job_status


def test_submit_job_ibm_backend():
    """Test submitting a job to IBM backend"""
    mock_response = {
        "job_id": "test-job-123",
        "status": "submitted"
    }
    
    with patch('httpx.post') as mock_post:
        mock_post.return_value = MagicMock(status_code=200, json=lambda: mock_response)
        
        result = submit_job("OPENQASM 3.0; qubit[2] q; h q[0]; cx q[0], q[1]; measure q[0] -> c[0];", "ibm")
        
        assert result["job_id"] == "test-job-123"
        assert result["status"] == "submitted"
        
        # Verify the request was made correctly
        mock_post.assert_called_once_with(
            "https://api.torino.ibm.com/hal/v1/jobs",
            json={"qasm": "OPENQASM 3.0; qubit[2] q; h q[0]; cx q[0], q[1]; measure q[0] -> c[0];", "shots": 1000},
            headers={"Content-Type": "application/json"}
        )

def test_submit_job_iqm_backend():
    """Test submitting a job to IQM backend"""
    mock_response = {
        "job_id": "test-job-456",
        "status": "submitted"
    }
    
    with patch('httpx.post') as mock_post:
        mock_post.return_value = MagicMock(status_code=200, json=lambda: mock_response)
        
        result = submit_job("OPENQASM 3.0; qubit[2] q; h q[0]; cx q[0], q[1]; measure q[0] -> c[0];", "iqm")
        
        assert result["job_id"] == "test-job-456"
        assert result["status"] == "submitted"
        
        # Verify the request was made correctly
        mock_post.assert_called_once_with(
            "https://api.garnet.iqm.com/hal/v1/jobs",
            json={"qasm": "OPENQASM 3.0; qubit[2] q; h q[0]; cx q[0], q[1]; measure q[0] -> c[0];", "shots": 1000},
            headers={"Content-Type": "application/json"}
        )

def test_submit_job_invalid_backend():
    """Test submitting a job to invalid backend"""
    with pytest.raises(ValueError):
        submit_job("test qasm", "invalid")

def test_get_job_status_ibm_backend():
    """Test getting job status from IBM backend"""
    mock_response = {
        "job_id": "test-job-123",
        "status": "completed",
        "result": {"counts": {"00": 500, "11": 500}}
    }
    
    with patch('httpx.get') as mock_get:
        mock_get.return_value = MagicMock(status_code=200, json=lambda: mock_response)
        
        result = get_job_status("test-job-123", "ibm")
        
        assert result["status"] == "completed"
        
        # Verify the request was made correctly
        mock_get.assert_called_once_with("https://api.torino.ibm.com/hal/v1/jobs/test-job-123")

def test_get_job_status_iqm_backend():
    """Test getting job status from IQM backend"""
    mock_response = {
        "job_id": "test-job-456",
        "status": "running"
    }
    
    with patch('httpx.get') as mock_get:
        mock_get.return_value = MagicMock(status_code=200, json=lambda: mock_response)
        
        result = get_job_status("test-job-456", "iqm")
        
        assert result["status"] == "running"
        
        # Verify the request was made correctly
        mock_get.assert_called_once_with("https://api.garnet.iqm.com/hal/v1/jobs/test-job-456")

def test_get_job_status_invalid_backend():
    """Test getting job status from invalid backend"""
    with pytest.raises(ValueError):
        get_job_status("test-job-123", "invalid")