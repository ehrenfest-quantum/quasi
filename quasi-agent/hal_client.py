import requests


def submit_job(qasm_str: str, backend: str):
    """Submit a QASM3 program to a HAL Contract backend.

    Args:
        qasm_str: The QASM3 source code as a string.
        backend: Identifier of the target backend (e.g., "ibm_torino").

    Returns:
        The job identifier returned by the HAL service.
    """
    payload = {
        "qasm": qasm_str,
        "backend": backend,
        "shots": 1024,
    }
    # In a real implementation this would be the HAL endpoint URL.
    response = requests.post("http://example.com/submit", json=payload)
    response.raise_for_status()
    return response.json().get("job_id")


def get_job_status(job_id: str):
    """Poll the HAL Contract service for the status of a submitted job.

    Args:
        job_id: The identifier of the job returned by ``submit_job``.

    Returns:
        The status string (e.g., "PENDING", "RUNNING", "COMPLETED").
    """
    url = f"http://example.com/status/{job_id}"
    response = requests.get(url)
    response.raise_for_status()
    return response.json().get("status")
