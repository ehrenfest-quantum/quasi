import pytest
import json
from unittest.mock import patch, Mock
from hal_client import submit_job


def test_submit_job_request_body_structure():
    """Test that submit_job sends a request with correct JSON structure."""
    # Mock the urllib.request.Request and urlopen
    with patch('hal_client.urllib.request.Request') as mock_request, \
         patch('hal_client.urllib.request.urlopen') as mock_urlopen:
        
        # Setup mock response
        mock_response = Mock()
        mock_response.read.return_value = json.dumps({"job_id": "test-123"}).encode('utf-8')
        mock_urlopen.return_value.__enter__.return_value = mock_response
        
        # Call the function
        result = submit_job(
            qasm_str="OPENQASM 3.0; qubit q; h q; measure q;",
            backend="ibm-torino",
            base_url="https://test.hal.quasi.quantum"
        )
        
        # Verify request was created with correct arguments
        mock_request.assert_called_once()
        
        # Get the call arguments
        call_args = mock_request.call_args[1]  # keyword args
        
        # Check method and headers
        assert call_args['method'] == 'POST'
        assert call_args['headers']['Content-Type'] == 'application/json'
        
        # Check the data payload structure
        data = json.loads(call_args['data'].decode('utf-8'))
        assert 'qasm' in data
        assert 'backend' in data
        assert data['qasm'] == "OPENQASM 3.0; qubit q; h q; measure q;"
        assert data['backend'] == "ibm-torino"
        
        # Check URL
        assert mock_request.call_args[0][0] == "https://test.hal.quasi.quantum/jobs"
        
        # Check result
        assert result == {"job_id": "test-123"}