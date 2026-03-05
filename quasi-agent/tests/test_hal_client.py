import pytest
from unittest.mock import patch, MagicMock
import json
import sys
import os

# Add parent directory to path for imports
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from hal_client import submit_job, get_job_status


def test_submit_job_request_body_schema():
    """Validate that the HTTP request body conforms to the HAL Contract JSON schema."""
    qasm_str = "OPENQASM 3.0; qubit[2] q;"
    backend = "ibm-torino"
    shots = 1024
    
    with patch('hal_client.requests.post') as mock_post:
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = {"job_id": "test-job-123"}
        mock_post.return_value = mock_response
        
        result = submit_job(qasm_str, backend, shots)
        
        # Verify the request was made
        mock_post.assert_called_once()
        
        # Get the request body
        call_args = mock_post.call_args
        request_body = call_args.kwargs.get('json', {})
        
        # Validate HAL Contract JSON schema fields
        assert 'qasm' in request_body, "Request body must contain 'qasm' field"
        assert 'backend' in request_body, "Request body must contain 'backend' field"
        assert 'shots' in request_body, "Request body must contain 'shots' field"
        assert 'format' in request_body, "Request body must contain 'format' field"
        
        # Validate field values
        assert request_body['qasm'] == qasm_str, "QASM string must match input"
        assert request_body['backend'] == backend, "Backend must match input"
        assert request_body['shots'] == shots, "Shots must match input"
        assert request_body['format'] == "qasm3", "Format must be qasm3"
        
        # Validate types
        assert isinstance(request_body['qasm'], str), "qasm must be a string"
        assert isinstance(request_body['backend'], str), "backend must be a string"
        assert isinstance(request_body['shots'], int), "shots must be an integer"
        assert isinstance(request_body['format'], str), "format must be a string"


def test_submit_job_default_shots():
    """Test that default shots value is used when not specified."""
    qasm_str = "OPENQASM 3.0; qubit[1] q;"
    backend = "iqm-garnet"
    
    with patch('hal_client.requests.post') as mock_post:
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = {"job_id": "test-job-456"}
        mock_post.return_value = mock_response
        
        result = submit_job(qasm_str, backend)
        
        request_body = mock_post.call_args.kwargs.get('json', {})
        assert request_body['shots'] == 1024, "Default shots should be 1024"


def test_get_job_status_request():
    """Validate that get_job_status makes correct HTTP GET request."""
    job_id = "test-job-123"
    
    with patch('hal_client.requests.get') as mock_get:
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = {"status": "completed", "job_id": job_id}
        mock_get.return_value = mock_response
        
        result = get_job_status(job_id)
        
        # Verify the request was made with correct URL
        mock_get.assert_called_once()
        call_args = mock_get.call_args
        assert f"jobs/{job_id}" in call_args.args[0], "URL must contain job_id"
        
        # Validate response
        assert result['status'] == "completed"
        assert result['job_id'] == job_id


def test_submit_job_returns_job_id():
    """Test that submit_job returns the job_id from the response."""
    qasm_str = "OPENQASM 3.0; qubit[1] q;"
    backend = "ibm-torino"
    expected_job_id = "hal-job-789"
    
    with patch('hal_client.requests.post') as mock_post:
        mock_response = MagicMock()
        mock_response.status_code = 200
        mock_response.json.return_value = {"job_id": expected_job_id, "status": "submitted"}
        mock_post.return_value = mock_response
        
        result = submit_job(qasm_str, backend)
        
        assert result['job_id'] == expected_job_id
        assert result['status'] == "submitted"
