import pytest
from quasi-agent.hal_client import submit_job, get_job_status

test_submit_job():
    result = submit_job('qasm3 ...', 'ibm')
    assert 'job_id' in result

test_get_job_status():
    job_id = submit_job('qasm3 ...', 'iqm')['job_id']
    status = get_job_status(job_id)
    assert status in ['success', 'running', 'error']