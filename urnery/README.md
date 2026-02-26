# urnery

Minimal QUASI urn registry service (FastAPI, file-backed).

## Endpoints

- `POST /urns`
- `GET /urns/{name}`
- `GET /urns/{name}/{version}`
- `GET /urns/{name}/{version}/download`
- `GET /urns/search?q=...`
- `GET /urns?page=1&page_size=20`

## Publish payload

```json
{
  "name": "grover-search",
  "version": "0.1.0",
  "description": "Grover search primitive",
  "urn_schema": "quasi.urn.v1",
  "entrypoint": "main",
  "program_cbor_hex": "a0"
}
```

## Storage layout

Data is stored under `urnery/store/` by default:

- `index.json`
- `{name}/{version}/program.cbor`
- `{name}/{version}/meta.json`

Override via `URNERY_STORE=/path/to/store`.

## Run

```bash
uvicorn urnery.main:app --reload --port 8090
```

## Tests

```bash
pytest -q urnery/tests/test_urnery_api.py
```

