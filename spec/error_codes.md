# QUASI HAL Contract Error Codes

This document defines the standard error response format for HAL Contract backends and lists the defined error codes.

## Error Response Format

The standard error response format is:

```json
{"error": "QUASI_ERR_XXX", "message": "...", "retryable": true/false}
```

Where:

*   `error`: A string representing the error code.
*   `message`: A human-readable message describing the error.
*   `retryable`: A boolean indicating whether the operation can be retried.

## Defined Error Codes

*   `QUASI_ERR_001`: Connectivity Error - Unable to connect to the QPU.
    `retryable`: `true`
*   `QUASI_ERR_002`: Queue Full - The QPU queue is full.
    `retryable`: `true`
*   `QUASI_ERR_003`: Shot Limit Exceeded - The requested number of shots exceeds the QPU's limit.
    `retryable`: `false`
*   `QUASI_ERR_004`: Hardware Fault - A hardware fault occurred on the QPU.
    `retryable`: `false`
*   `QUASI_ERR_005`: Job Timeout - The job timed out.
    `retryable`: `false`
*   `QUASI_ERR_006`: Invalid Circuit - The circuit is invalid.
    `retryable`: `false`