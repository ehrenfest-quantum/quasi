import unittest
from hal_client import submit_job, get_job_status
import jsonschema

class TestHALClient(unittest.TestCase):
    def test_submit_job(self):
        qasm_str = 'OPENQASM 3.0; qubit[2] q; h q[0]; cx q[0], q[1];'
        job_id = submit_job(qasm_str, 'ibm')
        self.assertIsNotNone(job_id)

    def test_get_job_status(self):
        job_id = '12345'
        status = get_job_status(job_id)
        self.assertIn(status, ['pending', 'running', 'completed'])

    def test_hal_contract_schema(self):
        qasm_str = 'OPENQASM 3.0; qubit[2] q; h q[0]; cx q[0], q[1];'
        payload = {'qasm': qasm_str}
        schema = {
            'type': 'object',
            'properties': {'qasm': {'type': 'string'}},
            'required': ['qasm']
        }
        jsonschema.validate(instance=payload, schema=schema)

if __name__ == '__main__':
    unittest.main()