import sys
import os
from pathlib import Path
from unittest.mock import patch
from tempfile import mkdtemp

# Redirect /home/vops paths to temp dir for testing
_tmp = mkdtemp(prefix="quasi-test-")
_original_path_init = Path.__new__

def _patched_path(cls, *args, **kwargs):
    obj = object.__new__(cls)
    return obj

# Monkey-patch at module level before server import
_orig_exists = Path.exists
_orig_read_text = Path.read_text
_orig_read_bytes = Path.read_bytes
_orig_write_bytes = Path.write_bytes
_orig_write_text = Path.write_text
_orig_mkdir = Path.mkdir
_orig_chmod = Path.chmod

def _safe_exists(self):
    if str(self).startswith("/home/vops"):
        safe = Path(_tmp) / str(self).lstrip("/")
        return _orig_exists(safe)
    return _orig_exists(self)

def _safe_read_bytes(self):
    if str(self).startswith("/home/vops"):
        safe = Path(_tmp) / str(self).lstrip("/")
        return _orig_read_bytes(safe)
    return _orig_read_bytes(self)

def _safe_write_bytes(self, data):
    if str(self).startswith("/home/vops"):
        safe = Path(_tmp) / str(self).lstrip("/")
        _orig_mkdir(safe.parent, parents=True, exist_ok=True)
        return _orig_write_bytes(safe, data)
    return _orig_write_bytes(self, data)

def _safe_read_text(self, *a, **kw):
    if str(self).startswith("/home/vops"):
        safe = Path(_tmp) / str(self).lstrip("/")
        return _orig_read_text(safe, *a, **kw)
    return _orig_read_text(self, *a, **kw)

def _safe_write_text(self, data, *a, **kw):
    if str(self).startswith("/home/vops"):
        safe = Path(_tmp) / str(self).lstrip("/")
        _orig_mkdir(safe.parent, parents=True, exist_ok=True)
        return _orig_write_text(safe, data, *a, **kw)
    return _orig_write_text(self, data, *a, **kw)

def _safe_mkdir(self, *a, **kw):
    if str(self).startswith("/home/vops"):
        safe = Path(_tmp) / str(self).lstrip("/")
        return _orig_mkdir(safe, *a, **kw)
    return _orig_mkdir(self, *a, **kw)

def _safe_chmod(self, mode, *a, **kw):
    if str(self).startswith("/home/vops"):
        safe = Path(_tmp) / str(self).lstrip("/")
        return _orig_chmod(safe, mode, *a, **kw)
    return _orig_chmod(self, mode, *a, **kw)

Path.exists = _safe_exists
Path.read_bytes = _safe_read_bytes
Path.write_bytes = _safe_write_bytes
Path.read_text = _safe_read_text
Path.write_text = _safe_write_text
Path.mkdir = _safe_mkdir
Path.chmod = _safe_chmod
