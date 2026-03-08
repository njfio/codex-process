from __future__ import annotations

import sys
from pathlib import Path


def ensure_local_sdk_src() -> Path:
    """Add sdk/python/src to sys.path so examples run without installing the package."""
    sdk_python_dir = Path(__file__).resolve().parents[1]
    src_dir = sdk_python_dir / "src"
    package_dir = src_dir / "codex_app_server"
    if not package_dir.exists():
        raise RuntimeError(f"Could not locate local SDK package at {package_dir}")

    src_str = str(src_dir)
    if src_str not in sys.path:
        sys.path.insert(0, src_str)
    return src_dir
