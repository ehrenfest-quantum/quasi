from fastapi.testclient import TestClient

from urnery.main import create_app


def _publish_payload(name: str = "grover-search", version: str = "0.1.0") -> dict:
    return {
        "name": name,
        "version": version,
        "description": "Grover search primitive",
        "urn_schema": "quasi.urn.v1",
        "entrypoint": "main",
        "program_cbor_hex": "a0",
    }


def test_publish_get_download_and_search(tmp_path):
    app = create_app(str(tmp_path / "store"))
    client = TestClient(app)

    publish = client.post("/urns", json=_publish_payload())
    assert publish.status_code == 201
    assert publish.json()["download_url"] == "/urns/grover-search/0.1.0/download"

    by_name = client.get("/urns/grover-search")
    assert by_name.status_code == 200
    assert len(by_name.json()["versions"]) == 1

    by_version = client.get("/urns/grover-search/0.1.0")
    assert by_version.status_code == 200
    assert by_version.json()["entrypoint"] == "main"

    download = client.get("/urns/grover-search/0.1.0/download")
    assert download.status_code == 200
    assert download.headers["content-type"].startswith("application/cbor")
    assert download.content == bytes.fromhex("a0")

    search = client.get("/urns/search", params={"q": "grover"})
    assert search.status_code == 200
    assert len(search.json()["items"]) == 1

    listing = client.get("/urns", params={"page": 1, "page_size": 10})
    assert listing.status_code == 200
    assert listing.json()["total"] == 1
    assert listing.json()["items"][0]["name"] == "grover-search"


def test_duplicate_publish_is_409(tmp_path):
    app = create_app(str(tmp_path / "store"))
    client = TestClient(app)

    assert client.post("/urns", json=_publish_payload()).status_code == 201
    dup = client.post("/urns", json=_publish_payload())
    assert dup.status_code == 409


def test_invalid_publish_is_422(tmp_path):
    app = create_app(str(tmp_path / "store"))
    client = TestClient(app)

    invalid = _publish_payload()
    invalid["program_cbor_hex"] = "zz"
    res = client.post("/urns", json=invalid)
    assert res.status_code == 422

    invalid2 = _publish_payload(name="Bad Name")
    res2 = client.post("/urns", json=invalid2)
    assert res2.status_code == 422

