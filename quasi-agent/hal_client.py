import requests
from typing import Dict, Optional

class HALClient:
    def __init__(self, base_url: str = "https://api.quantum.example.com"):
        self.base_url = base_url
        self.session = requests.Session()

    def submit_job(self, qasm_str: str, backend: str) -> str:
        """Submit a QASM3 job to the specified backend.
        
        Args:
            qasm_str: QASM3 program as a string
            backend: Target backend (e.g., 'ibm_torino', 'iqm_garnet')
            
        Returns:
            Job ID as a string
        """
        url = f"{self.base_url}/jobs"
        payload = {
            "qasm": qasm_str,
            "backend": backend,
            "shots": 1024  # Default shots
        }
        
        response = self.session.post(url, json=payload)
        response.raise_for_status()
        
        job_data = response.json()
        return job_data["job_id"]

    def get_job_status(self, job_id: str) -> Dict:
        """Get the status of a submitted job.
        
        Args:
            job_id: Job ID to check
            
        Returns:
            Dictionary containing job status and metadata
        """
        url = f"{self.base_url}/jobs/{job_id}"
        
        response = self.session.get(url)
        response.raise_for_status()
        
        return response.json()

# Convenience functions for direct usage
hal_client = HALClient()

def submit_job(qasm_str: str, backend: str) -> str:
    return hal_client.submit_job(qasm_str, backend)

def get_job_status(job_id: str) -> Dict:
    return hal_client.get_job_status(job_id)