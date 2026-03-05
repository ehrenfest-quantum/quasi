import unittest
from unittest.mock import patch, Mock
import jsonschema

from quasi_agent import hal_client


class TestHalClient(unittest.TestCase):
    def test_submit_job_payload_matches_schema(self):
        # Minimal HAL Contract schema for job submission
        schema = {
            "type": "object",
            "properties": {
                "qasm": {"type": "string"},
                "backend": {"type": "string"}
            },
            "required": ["qasm", "backend"],
            "additionalProperties": False
        }

        qasm_example = "OPENQASM 3.0; qubit[2] q; h q[0]; cx q[0], q[1];"
        backend = "ibm"

        captured_payload = {}

        def mock_post(url, json):
            nonlocal captured_payload
            captured_payload = json
            mock_resp = Mock()
            mock_resp.json.return_value = {"job_id": "12345"}
            mock_resp.raise_for_status = Mock()
            return mock_resp

        with patch('requests.post', side_effect=mock_post):
            result = hal_client.submit_job(qasm_example, backend)

        # Verify that the function returned the mocked response
        self.assertEqual(result, {"job_id": "12345"})
        # Validate the captured payload against the schema
        jsonschema.validate(instance=captured_payload, schema=schema)
        # Also assert the payload content is as expected
        self.assertEqual(captured_payload, {"qasm": qasm_example, "backend": backend})


if __name__ == "__main__":
    unittest.main()
