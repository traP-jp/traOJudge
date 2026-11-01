"""Read a finished sandbox's artifact without following paths outside its job."""
from pathlib import Path


def read_output(job: Path, path: Path, maximum: int = 8 * 1024 * 1024) -> bytes:
    # The sandbox must have exited before this check: no guest can race the read.
    resolved = path.resolve(strict=True)
    if (path.is_symlink() or not resolved.is_relative_to(job.resolve(strict=True))
            or not resolved.is_file() or resolved.stat().st_size > maximum):
        raise ValueError("invalid output")
    return resolved.read_bytes()
