import os
from typing import Optional

from fastapi import FastAPI, HTTPException, Query
from fastapi.responses import FileResponse

from urnery.models import PublishUrnRequest, PublishUrnResponse
from urnery.storage import UrnStore
from urnery.validation import (
    decode_hex_payload,
    validate_cbor,
    validate_name,
    validate_urn_schema,
    validate_version,
)


def create_app(store_dir: Optional[str] = None) -> FastAPI:
    app = FastAPI(title="Urnery", version="0.1.0")
    base = store_dir or os.getenv(
        "URNERY_STORE",
        os.path.join(os.path.dirname(__file__), "store"),
    )
    app.state.store = UrnStore(base)

    @app.post("/urns", response_model=PublishUrnResponse, status_code=201)
    def publish_urn(req: PublishUrnRequest) -> PublishUrnResponse:
        try:
            validate_name(req.name)
            validate_version(req.version)
            validate_urn_schema(req.urn_schema)
            payload = decode_hex_payload(req.program_cbor_hex)
            validate_cbor(payload)
        except ValueError as exc:
            raise HTTPException(status_code=422, detail=str(exc)) from exc

        try:
            meta = app.state.store.publish(
                name=req.name,
                version=req.version,
                description=req.description,
                urn_schema=req.urn_schema,
                entrypoint=req.entrypoint,
                payload=payload,
            )
        except FileExistsError as exc:
            raise HTTPException(status_code=409, detail=str(exc)) from exc

        return PublishUrnResponse(
            name=meta["name"],
            version=meta["version"],
            created_at=meta["created_at"],
            download_url=f"/urns/{meta['name']}/{meta['version']}/download",
        )

    @app.get("/urns/search")
    def search_urns(q: str = Query(..., min_length=1)) -> dict:
        return {"items": app.state.store.search(q), "query": q}

    @app.get("/urns")
    def list_urns(
        page: int = Query(default=1, ge=1),
        page_size: int = Query(default=20, ge=1, le=200),
    ) -> dict:
        return app.state.store.list_urns(page=page, page_size=page_size)

    @app.get("/urns/{name}")
    def get_urn(name: str) -> dict:
        urn = app.state.store.get_urn(name)
        if not urn:
            raise HTTPException(status_code=404, detail=f"urn '{name}' not found")
        return urn

    @app.get("/urns/{name}/{version}")
    def get_urn_version(name: str, version: str) -> dict:
        meta = app.state.store.get_version(name, version)
        if not meta:
            raise HTTPException(status_code=404, detail=f"urn '{name}@{version}' not found")
        return meta

    @app.get("/urns/{name}/{version}/download")
    def download_urn(name: str, version: str) -> FileResponse:
        artifact = app.state.store.artifact_file(name, version)
        if artifact is None:
            raise HTTPException(status_code=404, detail=f"urn '{name}@{version}' not found")
        filename = f"{name}-{version}.cbor"
        return FileResponse(path=artifact, media_type="application/cbor", filename=filename)

    return app


app = create_app()
