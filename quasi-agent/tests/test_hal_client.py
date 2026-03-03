import pytest
import json
from unittest.mock import AsyncMock, patch
from quasi_agent.hal_client import HALClient

@pytest.fixture
def mock_client():
    with patch('httpx.AsyncClient') as mock:
        client = HALClient()
        client.client = AsyncMock()
        yield client

@pytest.mark.asyncio
async def test_submit_job(mock_client):
    # Mock response
    mock_response = AsyncMock()
    mock_response.status_code = 200
    mock_response.json.return_value = {"job_id": "test-job-123"}
    mock_client.client.post.return_value = mock_response
    
    # Test data
    test_qasm = "OPENQASM 3.0; qubit[2] q; h q[0]; cx q[0], q[1]; measure q;"
    test_backend = "ibm_torino"
    
    # Call the method
    job_id = await mock_client.submit_job(test_qasm, test_backend)
    
    # Assertions
    assert job_id == "test-job-123"
    
    # Verify the request was made correctly
    mock_client.client.post.assert_awaited_once()
    args, kwargs = mock_client.client.post.call_args
    
    assert args[0] == "https://hal-contract.example.com/jobs"
    
    # Verify the request body
    request_body = kwargs['json']
    assert request_body['qasm'] == test_qasm
    assert request_body['backend'] == test_backend
    assert request_body['shots'] == 1000

@pytest.mark.asyncio
async def test_get_job_status(mock_client):
    # Mock response
    mock_response = AsyncMock()
    mock_response.status_code = 200
    mock_response.json.return_value = {
        "job_id": "test-job-123",
        "status": "completed",
        "result": {"counts": {"00": 500, "11": 500}}
    }
    mock_client.client.get.return_value = mock_response
    
    # Call the method
    status = await mock_client.get_job_status("test-job-123")
    
    # Assertions
    assert status["job_id"] == "test-job-123"
    assert status["status"] == "completed"
    
    # Verify the request was made correctly
    mock_client.client.get.assert_awaited_once_with(
        "https://hal-contract.example.com/jobs/test-job-123/status"
    )

@pytest.mark.asyncio
async def test_submit_job_http_error(mock_client):
    # Mock error response
    mock_response = AsyncMock()
    mock_response.status_code = 400
    mock_response.raise_for_status.side_effect = httpx.HTTPStatusError(
        "Bad Request", request=None, response=mock_response
    )
    mock_client.client.post.return_value = mock_response
    
    # Test that the error is propagated
    with pytest.raises(httpx.HTTPStatusError):
        await mock_client.submit_job("invalid qasm", "invalid_backend")

@pytest.mark.asyncio
async def test_context_manager():
    with patch('quasi_agent.hal_client.HALClient') as mock_client:
        mock_client.return_value = AsyncMock()
        
        async with HALClient() as client:
            pass
            
        client.close.assert_awaited_once()