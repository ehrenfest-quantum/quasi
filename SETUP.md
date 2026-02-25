# Setting Up the Development Environment

Follow these steps to set up the development environment for QUASI.

## Prerequisites

- Python 3.11
- pip
- virtualenv

## Steps

1. Clone the repository:
   ```sh
   git clone https://github.com/ehrenfest-quantum/quasi.git
   cd quasi
   ```
2. Create a virtual environment:
   ```sh
   python -m venv venv
   source venv/bin/activate
   ```
3. Install the required dependencies:
   ```sh
   pip install -r requirements.txt
   ```
4. Set up the database:
   ```sh
   python manage.py migrate
   ```
5. Start the development server:
   ```sh
   python manage.py runserver
   ```

## Additional Configuration

- Configure environment variables in a `.env` file.
- Set up any additional tools or services required for development.

## Troubleshooting

If you encounter any issues, please refer to the [Troubleshooting Guide](TROUBLESHOOTING.md) or contact us at [support@quasi.org](mailto:support@quasi.org).