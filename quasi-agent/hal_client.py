import requests
import json

def submit_job(qasm_str, backend):
    url = f'https://{backend}.com/jobs'
    headers = {'Content-Type': 'application/json'}
    data = {'qasm': qasm_str}
    response = requests.post(url, headers=headers, data=json.dumps(data))
    return response.json()['job_id']

def get_job_status(job_id):
    url = f'https://example.com/jobs/{job_id}'
    response = requests.get(url)
    return response.json()['status']