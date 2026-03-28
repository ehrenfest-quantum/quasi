"""Read/write/mutate rotation.toml preserving comments and formatting."""

import os
import tempfile

import tomlkit


DEFAULT_TOML_PATH = os.environ.get(
    "ROTATION_TOML_PATH", "/home/vops/quasi-senate-rotation.toml"
)


def load_rotation(path=None):
    """Load rotation.toml as a TOMLDocument (preserves comments)."""
    path = path or DEFAULT_TOML_PATH
    with open(path, "r") as f:
        return tomlkit.load(f)


def save_rotation(path, doc):
    """Atomic write: write to .tmp then rename."""
    tmp_fd, tmp_path = tempfile.mkstemp(
        dir=os.path.dirname(path), suffix=".tmp"
    )
    try:
        with os.fdopen(tmp_fd, "w") as f:
            tomlkit.dump(doc, f)
        os.rename(tmp_path, path)
    except Exception:
        try:
            os.unlink(tmp_path)
        except OSError:
            pass
        raise


def get_existing_models(doc):
    """Return set of (provider, model) tuples from rotation entries."""
    entries = doc.get("rotation", [])
    return {(e["provider"], e["model"]) for e in entries}


def get_existing_ids(doc):
    """Return set of model IDs."""
    return {e["id"] for e in doc.get("rotation", [])}


def append_entry(doc, entry):
    """Append a new [[rotation]] entry to the document."""
    if "rotation" not in doc:
        doc["rotation"] = tomlkit.aot()

    item = tomlkit.table()
    for key in ["id", "model", "provider", "license", "origin", "roles"]:
        if key in entry:
            item.add(key, entry[key])
    if entry.get("quarantined"):
        item.add("quarantined", True)
    if entry.get("max_tokens"):
        item.add("max_tokens", entry["max_tokens"])

    doc["rotation"].append(item)


def update_field(doc, model_id, key, value):
    """Update a single field on the entry matching model_id."""
    for entry in doc.get("rotation", []):
        if entry["id"] == model_id:
            entry[key] = value
            return True
    return False
