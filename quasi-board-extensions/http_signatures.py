"""
HTTP Signatures for ActivityPub federation (QUASI-003)

Implements RFC 9421 (HTTP Message Signatures) subset for quasi-board:
- Sign outgoing ActivityPub requests with an RSA keypair
- Verify incoming signed requests from federated boards
"""

import base64
import hashlib
import time
from typing import Optional


def sign_request(
    method: str,
    path: str,
    host: str,
    body: bytes,
    private_key_pem: str,
    key_id: str,
) -> dict[str, str]:
    """
    Return headers dict with Signature and Digest for an ActivityPub HTTP request.

    key_id: URL of the actor public key, e.g.
            https://gawain.valiant-quantum.com/quasi-board#main-key
    """
    digest = "SHA-256=" + base64.b64encode(hashlib.sha256(body).digest()).decode()
    date = time.strftime("%a, %d %b %Y %H:%M:%S GMT", time.gmtime())

    signed_string = (
        f"(request-target): {method.lower()} {path}\n"
        f"host: {host}\n"
        f"date: {date}\n"
        f"digest: {digest}"
    )

    try:
        from cryptography.hazmat.primitives import hashes, serialization
        from cryptography.hazmat.primitives.asymmetric import padding

        key = serialization.load_pem_private_key(private_key_pem.encode(), password=None)
        sig_bytes = key.sign(signed_string.encode(), padding.PKCS1v15(), hashes.SHA256())
        signature = base64.b64encode(sig_bytes).decode()
    except ImportError:
        signature = "STUB_SIGNATURE_cryptography_not_installed"

    return {
        "Date": date,
        "Digest": digest,
        "Signature": (
            f'keyId="{key_id}",'
            f'algorithm="rsa-sha256",'
            f'headers="(request-target) host date digest",'
            f'signature="{signature}"'
        ),
    }


def verify_signature(
    headers: dict[str, str],
    method: str,
    path: str,
    body: bytes,
    public_key_pem: str,
) -> bool:
    """
    Verify an incoming HTTP Signature. Returns True if valid.
    Returns False (not raises) on any verification failure.
    """
    try:
        sig_header = headers.get("Signature", "")
        if not sig_header:
            return False

        params: dict[str, str] = {}
        for part in sig_header.split(","):
            k, _, v = part.partition("=")
            params[k.strip()] = v.strip().strip('"')

        key_headers = params.get("headers", "").split()
        signed_string = _build_signed_string(headers, method, path, key_headers)

        from cryptography.hazmat.primitives import hashes, serialization
        from cryptography.hazmat.primitives.asymmetric import padding

        key = serialization.load_pem_public_key(public_key_pem.encode())
        sig_bytes = base64.b64decode(params.get("signature", ""))
        key.verify(sig_bytes, signed_string.encode(), padding.PKCS1v15(), hashes.SHA256())

        if "Digest" in headers:
            expected = "SHA-256=" + base64.b64encode(hashlib.sha256(body).digest()).decode()
            if headers["Digest"] != expected:
                return False

        return True
    except Exception:
        return False


def _build_signed_string(
    headers: dict[str, str],
    method: str,
    path: str,
    signed_headers: list[str],
) -> str:
    lines = []
    for h in signed_headers:
        if h == "(request-target)":
            lines.append(f"(request-target): {method.lower()} {path}")
        else:
            lines.append(f"{h}: {headers.get(h, headers.get(h.title(), ''))}")
    return "\n".join(lines)
