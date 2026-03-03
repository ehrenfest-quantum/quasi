import requests
import json

class HalClient:
    def __init__(self, backend_url):
        self.backend_url = backend_url
        self.headers = {'Content-Type': 'application/json'}

    def submit_job(self, qasm_str, backend):
        url = f'{self.backend_url}/job'
        data = {'qasm': qasm_str, 'backend': backend}
        response = requests.post(url, json=data, headers=self.headers)
        if response.status_code == 200:
            return response.json()
        else:
            raise Exception(f'Failed to submit job: {response.text}')

    def get_job_status(self, job_id):
        url = f'{self.backend_url}/job/{job_id}'
        response = requests.get(url, headers=self.headers)
        if response.status_code == 200:
            return response.json()
        else:
            raise Exception(f'Failed to get job status: {response.text}')

# Example usage (not part of the minimal requirement, but for completeness)
# if __name__ == '__main__':
#     client = HalClient('https://example.com/hal')
#     job_id = client.submit_job('...', 'ibm_torino')
#     print(client.get_job_status(job_id))
