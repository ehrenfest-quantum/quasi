import unittest
from quasi_agent.hal_client import submit_job, get_job_status
import jsonschema
from jsonschema import validate

class TestHALClient(unittest.TestCase):
    def test_submit_job_schema(self):
        qasm_str = 'OPENQASM 3.0; qubit[2] q; h q[0]; cx q[0], q[1];'
        backend = 'example-backend.com'
        # Mocking the requests.post call
        import requests
        original_post = requests.post
        requests.post = lambda *args, **kwargs: MockResponse({'job_id': '1234'}, 201)
        job_id = submit_job(qasm_str, backend)
        requests.post = original_post
        self.assertEqual(job_id, '1234')
        # Validate the request body schema
        payload = {'qasm': qasm_str}
        hal_contract_schema = {
            'type': 'object',
            'properties': {
                'qasm': {'type': 'string'}
            },
            'required': ['qasm']
        }
        validate(instance=payload, schema=hal_contract_schema)

    def test_get_job_status_schema(self):
        job_id = '1234'
        backend = 'example-backend.com'
        # Mocking the requests.get call
        import requests
        original_get = requests.get
        requests.get = lambda *args, **kwargs: MockResponse({'status': 'COMPLETED'}, 200)
        status = get_job_status(job_id, backend)
        requests.get = original_get
        self.assertEqual(status, 'COMPLETED')
        # Validate the response schema
        response = {'status': 'COMPLETED'}
        hal_contract_schema = {
            'type': 'object',
            'properties': {
                'status': {'type': 'string'}
            },
            'required': ['status']
        }
        validate(instance=response, schema=hal_contract_schema)

class MockResponse:
    def __init__(self, json_data, status_code):
        self.json_data = json_data
        self.status_code = status_code
        self.text = json.dumps(json_data)

    def json(self):
        return self.json_data