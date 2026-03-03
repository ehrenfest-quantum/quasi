import json
import urllib.request
import urllib.error
from typing import Optional, Dict, Any


class HalClient:
    """Client for HAL Contract endpoints."""
    
    def __init__(self, base_url: str):
        self.base_url = base_url.rstrip('/')
    
    def submit_job(self, qasm_str: str, backend: str) -> Dict[str, Any]:
        """Submit a QASM3 job to the HAL Contract endpoint.
        
        Args:
            qasm_str: OpenQASM 3 program string
            backend: Target backend name (e.g., 'ibm-torino', 'iqm-garnet')
            
        Returns:
            Response JSON containing job_id
            
        Raises:
            urllib.error.HTTPError: If submission fails
        """
        url = f"{self.base_url}/jobs"
        
        payload = {
            "qasm": qasm_str,
            "backend": backend
        }
        
        data = json.dumps(payload).encode('utf-8')
        headers = {
            'Content-Type': 'application/json'
        }
        
        req = urllib.request.Request(url, data=data, headers=headers, method='POST')
        
        try:
            with urllib.request.urlopen(req) as response:
                return json.loads(response.read().decode('utf-8'))
        except urllib.error.HTTPError as e:
            raise e

    def get_job_status(self, job_id: str) -> Dict[str, Any]:
        """Get the status of a submitted job.
        
        Args:
            job_id: The job identifier returned by submit_job
            
        Returns:
            Response JSON with status and optionally results
            
        Raises:
            urllib.error.HTTPError: If request fails
        """
        url = f"{self.base_url}/jobs/{job_id}"
        
        req = urllib.request.Request(url, method='GET')
        
        try:
            with urllib.request.urlopen(req) as response:
                return json.loads(response.read().decode('utf-8'))
        except urllib.error.HTTPError as e:
            raise e


def submit_job(qasm_str: str, backend: str, base_url: str = "https://hal.quasi.quantum") -> Dict[str, Any]:
    """Convenience function to submit a job to the HAL Contract endpoint.
    
    Args:
        qasm_str: OpenQASM 3 program string
        backend: Target backend name
        base_url: Base URL of the HAL Contract server
        
    Returns:
        Response JSON containing job_id
    """
    client = HalClient(base_url)
    return client.submit_job(qasm_str, backend)


def get_job_status(job_id: str, base_url: str = "https://hal.quasi.quantum") -> Dict[str, Any]:
    """Convenience function to get job status from the HAL Contract endpoint.
    
    Args:
        job_id: The job identifier returned by submit_job
        base_url: Base URL of the HAL Contract server
        
    Returns:
        Response JSON with status and optionally results
    """
    client = HalClient(base_url)
    return client.get_job_status(job_id)