import json
from datetime import datetime, timezone
from pathlib import Path
from threading import Lock
from typing import Optional, Union


class UrnStore:
    def __init__(self, root: Union[str, Path]):
        self.root = Path(root)
        self.root.mkdir(parents=True, exist_ok=True)
        self.index_path = self.root / "index.json"
        self._lock = Lock()
        self._init_index()

    def _init_index(self) -> None:
        if not self.index_path.exists():
            self._write_index({"urns": {}})

    def _read_index(self) -> dict:
        return json.loads(self.index_path.read_text(encoding="utf-8"))

    def _write_index(self, data: dict) -> None:
        temp = self.index_path.with_suffix(".tmp")
        temp.write_text(json.dumps(data, indent=2, sort_keys=True), encoding="utf-8")
        temp.replace(self.index_path)

    def publish(
        self,
        *,
        name: str,
        version: str,
        description: str,
        urn_schema: str,
        entrypoint: str,
        payload: bytes,
    ) -> dict:
        with self._lock:
            index = self._read_index()
            urns = index.setdefault("urns", {})
            item = urns.setdefault(name, {"versions": {}})
            versions = item.setdefault("versions", {})
            if version in versions:
                raise FileExistsError(f"{name}@{version} already exists")

            created_at = datetime.now(timezone.utc).isoformat()
            target = self.root / name / version
            target.mkdir(parents=True, exist_ok=True)
            artifact_path = target / "program.cbor"
            metadata_path = target / "meta.json"
            artifact_path.write_bytes(payload)

            metadata = {
                "name": name,
                "version": version,
                "description": description,
                "urn_schema": urn_schema,
                "entrypoint": entrypoint,
                "created_at": created_at,
                "artifact_path": str(artifact_path.relative_to(self.root)),
            }
            metadata_path.write_text(json.dumps(metadata, indent=2), encoding="utf-8")
            versions[version] = metadata
            self._write_index(index)
            return metadata

    def get_urn(self, name: str) -> Optional[dict]:
        index = self._read_index()
        urn = index.get("urns", {}).get(name)
        if not urn:
            return None
        versions = sorted(urn.get("versions", {}).values(), key=lambda x: x["version"])
        return {"name": name, "versions": versions}

    def get_version(self, name: str, version: str) -> Optional[dict]:
        index = self._read_index()
        return index.get("urns", {}).get(name, {}).get("versions", {}).get(version)

    def list_urns(self, page: int, page_size: int) -> dict:
        index = self._read_index()
        all_names = sorted(index.get("urns", {}).keys())
        total = len(all_names)
        start = (page - 1) * page_size
        end = start + page_size
        page_names = all_names[start:end]
        items = []
        for name in page_names:
            versions = index["urns"][name].get("versions", {})
            latest = sorted(versions.values(), key=lambda x: x["version"])[-1] if versions else None
            items.append(
                {
                    "name": name,
                    "versions": len(versions),
                    "latest_version": latest["version"] if latest else None,
                }
            )
        return {"items": items, "page": page, "page_size": page_size, "total": total}

    def search(self, query: str) -> list[dict]:
        q = query.lower().strip()
        if not q:
            return []
        index = self._read_index()
        matches = []
        for name, item in index.get("urns", {}).items():
            for version, meta in item.get("versions", {}).items():
                haystack = f"{name} {version} {meta.get('description', '')}".lower()
                if q in haystack:
                    matches.append(meta)
        return sorted(matches, key=lambda x: (x["name"], x["version"]))

    def artifact_file(self, name: str, version: str) -> Optional[Path]:
        meta = self.get_version(name, version)
        if not meta:
            return None
        path = self.root / meta["artifact_path"]
        return path if path.exists() else None
