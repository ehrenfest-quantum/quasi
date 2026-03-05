import requests


def submit_job(qasm_str: str, backend: str):
    """Submit a QASM3 program to the given backend via the HAL Contract.

    Args:
        qasm_str: The QASM3 program as a string.
        backend: Identifier of the backend (e.g., "ibm", "iqm").

    Returns:
        The parsed JSON response from the HAL endpoint.
    """
    payload = {"qasm": qasm_str, "backend": backend}
    # In a real implementation the URL would be derived from the backend configuration.
    url = f"https://{backend}.example.com/submit"
    response = requests.post(url, json=payload)
    response.raise_for_status()
    return response.json()


def get_job_status(job_id: str, backend: str = "default"):
    """Retrieve the status of a previously submitted job.

    Args:
        job_id: Identifier returned by ``submit_job``.
        backend: Backend identifier used to build the status URL.

    Returns:
        The parsed JSON status response.
    """
    url = f"https://{backend}.example.com/status/{job_id}"
    response = requests.get(url)
    response.raise_for_status()
    return response.json()
