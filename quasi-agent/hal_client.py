import requests
import json
from typing import Dict, Any, Optional

# HAL Contract API base URL - configurable for different backends
HAL_API_BASE = "https://hal.quasi.io/v1"


def submit_job(qasm_str: str, backend: str, shots: int = 1024) -> Dict[str, Any]:
    """
    Submit a QASM3 job to the HAL Contract API.
    
    Args:
        qasm_str: QASM3 program string to execute
        backend: Target backend identifier (e.g., "ibm-torino", "iqm-garnet")
        shots: Number of measurement shots (default: 1024)
    
    Returns:
        Dict containing job_id and submission metadata
    
    Raises:
        requests.RequestException: If the API request fails
    """
    url = f"{HAL_API_BASE}/jobs"
    
    # HAL Contract JSON schema for job submission
    payload = {
        "qasm": qasm_str,
        "backend": backend,
        "shots": shots,
        "format": "qasm3"
    }
    
    response = requests.post(url, json=payload)
    response.raise_for_status()
    
    return response.json()


def get_job_status(job_id: str) -> Dict[str, Any]:
    """
    Get the status of a submitted job.
    
    Args:
        job_id: The job identifier returned from submit_job
    
    Returns:
        Dict containing job status and metadata
    
    Raises:
        requests.RequestException: If the API request fails
    """
    url = f"{HAL_API_BASE}/jobs/{job_id}"
    
    response = requests.get(url)
    response.raise_for_status()
    
    return response.json()


def get_job_results(job_id: str) -> Dict[str, Any]:
    """
    Retrieve results for a completed job.
    
    Args:
        job_id: The job identifier
    
    Returns:
        Dict containing measurement results
    
    Raises:
        requests.RequestException: If the API request fails
    """
    url = f"{HAL_API_BASE}/jobs/{job_id}/results"
    
    response = requests.get(url)
    response.raise_for_status()
    
    return response.json()
