import unittest
from unittest.mock import patch, Mock
from hal_client import submit_job, get_job_status


class TestHALClient(unittest.TestCase):

    @patch('requests.post')
    def test_submit_job(self, mock_post):
        mock_response = Mock()
        mock_response.status_code = 200
        mock_response.json.return_value = {'job_id': '12345'}
        mock_post.return_value = mock_response

        job_id = submit_job('qasm_string', 'ibm-torino')
        self.assertEqual(job_id, '12345')

    @patch('requests.get')
    def test_get_job_status(self, mock_get):
        mock_response = Mock()
        mock_response.status_code = 200
        mock_response.json.return_value = {'status': 'COMPLETED'}
        mock_get.return_value = mock_response

        status = get_job_status('12345', 'ibm-torino')
        self.assertEqual(status, 'COMPLETED')


if __name__ == '__main__':
    unittest.main()