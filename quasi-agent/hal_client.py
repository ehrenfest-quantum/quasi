import requests
import json

def submit_job(qasm_str, backend):
    url = f'https://{backend}/jobs'
    payload = {'qasm': qasm_str}
    response = requests.post(url, json=payload)
    if response.status_code == 201:
        return response.json()['job_id']
    else:
        raise Exception(f'Failed to submit job: {response.text}')

def get_job_status(job_id, backend):
    url = f'https://{backend}/jobs/{job_id}'
    response = requests.get(url)
    if response.status_code == 200:
        return response.json()['status']
    else:
        raise Exception(f'Failed to get job status: {response.text}')