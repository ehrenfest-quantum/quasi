import pytest
from unittest.mock import Mock, patch
from quasi_agent.hal_client import HALClient, submit_job, get_job_status

@patch('requests.Session')
def test_submit_job(mock_session):
    # Setup mock response
    mock_response = Mock()
    mock_response.status_code = 201
    mock_response.json.return_value = {"job_id": "test-job-123"}
    
    # Configure mock session
    mock_session.return_value.post.return_value = mock_response
    
    # Test
    client = HALClient()
    job_id = client.submit_job("OPENQASM 3.0; qubit[2] q; h q[0]; cx q[0], q[1];", "ibm_torino")
    
    # Verify
    assert job_id == "test-job-123"
    mock_session.return_value.post.assert_called_once()
    
    # Check request payload structure
    call_args = mock_session.return_value.post.call_args
    request_payload = call_args[1]['json']
    assert 'qasm' in request_payload
    assert 'backend' in request_payload
    assert 'shots' in request_payload
    assert request_payload['backend'] == 'ibm_torino'

@patch('requests.Session')
def test_get_job_status(mock_session):
    # Setup mock response
    mock_response = Mock()
    mock_response.status_code = 200
    mock_response.json.return_value = {
        "job_id": "test-job-123",
        "status": "completed",
        "result": {"counts": {"00": 512, "11": 512}}
    }
    
    # Configure mock session
    mock_session.return_value.get.return_value = mock_response
    
    # Test
    client = HALClient()
    status = client.get_job_status("test-job-123")
    
    # Verify
    assert status["status"] == "completed"
    assert "result" in status
    mock_session.return_value.get.assert_called_once()

@patch('quasi_agent.hal_client.hal_client')
def test_module_level_functions(mock_client):
    # Test convenience functions
    mock_client.submit_job.return_value = "test-job-456"
    mock_client.get_job_status.return_value = {"status": "running"}
    
    job_id = submit_job("OPENQASM 3.0; x $0;", "iqm_garnet")
    status = get_job_status(job_id)
    
    assert job_id == "test-job-456"
    assert status["status"] == "running"
    mock_client.submit_job.assert_called_once()
    mock_client.get_job_status.assert_called_once()