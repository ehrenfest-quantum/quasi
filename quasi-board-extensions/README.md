# quasi-board-extensions: HTTP Signatures (QUASI-003)

HTTP Message Signatures for ActivityPub federation.

## Usage

```python
from http_signatures import sign_request, verify_signature

# Sign an outgoing request
headers = sign_request(
    method="POST",
    path="/inbox",
    host="other-board.example.com",
    body=b'{"type": "Follow", ...}',
    private_key_pem=MY_PRIVATE_KEY,
    key_id="https://gawain.valiant-quantum.com/quasi-board#main-key",
)
```

## Dependencies

- `cryptography` (optional — stub mode without it)
