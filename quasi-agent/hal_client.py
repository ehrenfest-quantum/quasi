'''HAL Contract client bindings for IBM/IQM backends.'''

import requests
from typing import Any, Dict

def _backend_url(backend: str) -> str:
    """Return a placeholder URL for the given backend.

    In a real implementation this would map to actual service endpoints.
    """
    return f"https://{backend}.example.com/api/v1"

def submit_job(qasm_str: str, backend: str) -> str:
    """Submit a QASM3 program to a HAL Contract endpoint.

    Args:
        qasm_str: The QASM3 source code.
        backend: Identifier of the target backend (e.g., "ibm", "iqm").

    Returns:
        The job identifier returned by the service.
    """
    url = f"{_backend_url(backend)}/jobs"
    payload: Dict[str, Any] = {
        "qasm3": qasm_str,
        "backend": backend,
        "metadata": {}
    }
    response = requests.post(url, json=payload)
    response.raise_for_status()
    data = response.json()
    return data.get("job_id", "")

def get_job_status(job_id: str) -> Dict[str, Any]:
    """Retrieve the status of a previously submitted job.

    Args:
        job_id: The identifier of the job.

    Returns:
        The JSON payload describing the job status.
    """
    url = f"https://hal.example.com/api/v1/jobs/{job_id}"
    response = requests.get(url)
    response.raise_for_status()
    return response.json()
