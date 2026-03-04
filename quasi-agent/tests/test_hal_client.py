import unittest
import pathlib
import sys
from unittest.mock import patch, Mock

# Ensure the quasi-agent directory is on the import path
sys.path.append(str(pathlib.Path(__file__).resolve().parents[1]))

from hal_client import submit_job

class TestHalClient(unittest.TestCase):
    @patch('hal_client.requests.post')
    def test_submit_job_payload(self, mock_post):
        # Mock response object
        mock_resp = Mock()
        mock_resp.json.return_value = {"job_id": "test123"}
        mock_resp.raise_for_status = Mock()
        mock_post.return_value = mock_resp

        qasm = "OPENQASM 3; // dummy"
        backend = "ibm"
        job_id = submit_job(qasm, backend)

        # Verify HTTP call
        mock_post.assert_called_once()
        _, kwargs = mock_post.call_args
        payload = kwargs.get('json')
        self.assertIsNotNone(payload)
        self.assertEqual(payload.get('qasm3'), qasm)
        self.assertEqual(payload.get('backend'), backend)

        # Verify returned job id
        self.assertEqual(job_id, "test123")

if __name__ == '__main__':
    unittest.main()
