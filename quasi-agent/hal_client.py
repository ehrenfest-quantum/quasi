import requests


def submit_job(qasm_str, backend):
    url = f'https://{backend}/jobs'
    headers = {'Content-Type': 'application/json'}
    data = {
        'qasm': qasm_str,
        'backend': backend
    }
    response = requests.post(url, json=data, headers=headers)
    response.raise_for_status()
    return response.json()['job_id']


def get_job_status(job_id, backend):
    url = f'https://{backend}/jobs/{job_id}'
    response = requests.get(url)
    response.raise_for_status()
    return response.json()['status']