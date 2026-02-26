from pydantic import BaseModel, Field


class PublishUrnRequest(BaseModel):
    name: str = Field(min_length=1, max_length=128)
    version: str = Field(min_length=1, max_length=64)
    description: str = Field(default="", max_length=1024)
    urn_schema: str = Field(default="quasi.urn.v1", min_length=1, max_length=128)
    entrypoint: str = Field(default="main", min_length=1, max_length=128)
    program_cbor_hex: str = Field(min_length=2)


class PublishUrnResponse(BaseModel):
    name: str
    version: str
    created_at: str
    download_url: str

