import httpx
import json
from typing import Dict, Optional, Union

class HALClient:
    """Client for interacting with HAL Contract endpoints.
    
    Implements the HAL Contract specification for job submission and status checking.
    """
    
    def __init__(self, base_url: str = "https://hal-contract.example.com"):
        self.base_url = base_url
        self.client = httpx.AsyncClient()
    
    async def submit_job(self, qasm_str: str, backend: str) -> str:
        """Submit a QASM3 job to the specified backend.
        
        Args:
            qasm_str: QASM3 program as a string
            backend: Target backend identifier (e.g., 'ibm_torino', 'iqm_garnet')
            
        Returns:
            Job ID as a string
            
        Raises:
            httpx.HTTPStatusError: If the request fails
        """
        url = f"{self.base_url}/jobs"
        payload = {
            "qasm": qasm_str,
            "backend": backend,
            "shots": 1000  # Default shot count
        }
        
        response = await self.client.post(url, json=payload)
        response.raise_for_status()
        return response.json()["job_id"]
    
    async def get_job_status(self, job_id: str) -> Dict[str, Union[str, int, float]]:
        """Get the status of a submitted job.
        
        Args:
            job_id: The job ID to check
            
        Returns:
            Dictionary containing job status information
            
        Raises:
            httpx.HTTPStatusError: If the request fails
        """
        url = f"{self.base_url}/jobs/{job_id}/status"
        response = await self.client.get(url)
        response.raise_for_status()
        return response.json()
    
    async def close(self):
        """Close the HTTP client."""
        await self.client.aclose()

# Convenience functions for common use cases
async def submit_job(qasm_str: str, backend: str) -> str:
    """Submit a job using a new client instance."""
    async with HALClient() as client:
        return await client.submit_job(qasm_str, backend)

async def get_job_status(job_id: str) -> Dict[str, Union[str, int, float]]:
    """Get job status using a new client instance."""
    async with HALClient() as client:
        return await client.get_job_status(job_id)

# Make HALClient usable as a context manager
HALClient.__aenter__ = lambda self: self
HALClient.__aexit__ = lambda self, exc_type, exc_val, exc_tb: self.close()