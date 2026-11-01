#!/usr/bin/env python3
"""Exercise the host boundary with guest-controlled output paths."""
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "pool"))
from artifacts import read_output


class ArtifactBoundary(unittest.TestCase):
    def test_completed_output(self):
        with tempfile.TemporaryDirectory() as temp:
            job = Path(temp)
            output = job / "binary"
            output.write_bytes(b"native artifact")
            self.assertEqual(read_output(job, output), b"native artifact")

    def test_final_symlink_is_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            job = Path(temp)
            (job / "target").write_bytes(b"data")
            (job / "binary").symlink_to(job / "target")
            with self.assertRaises(ValueError):
                read_output(job, job / "binary")

    def test_parent_symlink_cannot_read_host_file(self):
        with tempfile.TemporaryDirectory() as temp:
            parent = Path(temp)
            job = parent / "job"
            job.mkdir()
            (parent / "binary").write_bytes(b"host-private")
            (job / "outputs").symlink_to(parent, target_is_directory=True)
            with self.assertRaises(ValueError):
                read_output(job, job / "outputs/binary")

    def test_non_file_and_oversize_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            job = Path(temp)
            with self.assertRaises(ValueError):
                read_output(job, job)
            (job / "binary").write_bytes(b"too large")
            with self.assertRaises(ValueError):
                read_output(job, job / "binary", maximum=3)


if __name__ == "__main__":
    unittest.main()
