import re


NAME_RE = re.compile(r"^[a-z0-9][a-z0-9._-]{0,127}$")
SEMVER_RE = re.compile(r"^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$")


def validate_name(name: str) -> None:
    if not NAME_RE.match(name):
        raise ValueError("name must match ^[a-z0-9][a-z0-9._-]{0,127}$")


def validate_version(version: str) -> None:
    if not SEMVER_RE.match(version):
        raise ValueError("version must be semver-like (e.g. 1.2.3)")


def validate_urn_schema(urn_schema: str) -> None:
    if not urn_schema.startswith("quasi.urn."):
        raise ValueError("urn_schema must start with 'quasi.urn.'")


def decode_hex_payload(payload_hex: str) -> bytes:
    try:
        return bytes.fromhex(payload_hex)
    except ValueError as exc:
        raise ValueError("program_cbor_hex must be valid hex") from exc


def validate_cbor(data: bytes) -> None:
    if not data:
        raise ValueError("program_cbor_hex decodes to empty payload")
    end = _parse_cbor_item(data, 0)
    if end != len(data):
        raise ValueError("program_cbor_hex has trailing bytes after CBOR item")


def _parse_cbor_item(data: bytes, pos: int) -> int:
    if pos >= len(data):
        raise ValueError("unexpected end of CBOR payload")
    initial = data[pos]
    pos += 1
    major = initial >> 5
    addl = initial & 0x1F
    value, pos = _read_additional(data, pos, addl)

    if major in (0, 1):
        return pos
    if major in (2, 3):
        end = pos + value
        if end > len(data):
            raise ValueError("invalid CBOR length")
        return end
    if major == 4:
        for _ in range(value):
            pos = _parse_cbor_item(data, pos)
        return pos
    if major == 5:
        for _ in range(value):
            pos = _parse_cbor_item(data, pos)
            pos = _parse_cbor_item(data, pos)
        return pos
    if major == 6:
        return _parse_cbor_item(data, pos)
    if major == 7:
        if addl in (20, 21, 22, 23):
            return pos
        if addl == 24:
            if pos + 1 > len(data):
                raise ValueError("invalid CBOR simple value")
            return pos + 1
        if addl == 25:
            if pos + 2 > len(data):
                raise ValueError("invalid CBOR float16")
            return pos + 2
        if addl == 26:
            if pos + 4 > len(data):
                raise ValueError("invalid CBOR float32")
            return pos + 4
        if addl == 27:
            if pos + 8 > len(data):
                raise ValueError("invalid CBOR float64")
            return pos + 8
        if addl in (28, 29, 30, 31):
            raise ValueError("unsupported/invalid CBOR simple value")
        return pos
    raise ValueError("unknown CBOR major type")


def _read_additional(data: bytes, pos: int, addl: int) -> tuple[int, int]:
    if addl < 24:
        return addl, pos
    if addl == 24:
        if pos + 1 > len(data):
            raise ValueError("truncated CBOR uint8")
        return data[pos], pos + 1
    if addl == 25:
        if pos + 2 > len(data):
            raise ValueError("truncated CBOR uint16")
        return int.from_bytes(data[pos:pos + 2], "big"), pos + 2
    if addl == 26:
        if pos + 4 > len(data):
            raise ValueError("truncated CBOR uint32")
        return int.from_bytes(data[pos:pos + 4], "big"), pos + 4
    if addl == 27:
        if pos + 8 > len(data):
            raise ValueError("truncated CBOR uint64")
        return int.from_bytes(data[pos:pos + 8], "big"), pos + 8
    if addl == 31:
        raise ValueError("indefinite-length CBOR is not supported")
    raise ValueError("invalid CBOR additional info")

