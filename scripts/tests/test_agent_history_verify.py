"""Isolated guards keep scale artifacts truthful before expensive full-scale runs.

Failure modes specified before the fix: missing .git aborts verification before the
CLI runs; absent provenance is misreported as a clean checkout; or the fallback
loses the source/binary fingerprints needed to identify an offloaded build.
Search success must reject omitted middle events, duplicate tail events and
incomplete coverage even when an otherwise plausible count is returned.
"""

import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


RUNNER = Path(__file__).resolve().parents[1] / "agent-history-verify.py"
SPEC = importlib.util.spec_from_file_location("agent_history_verify", RUNNER)
verify = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verify)


class SourceArchiveProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="agent-history-provenance-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.source = self.root / "src-tauri/crates/agent-history/src/lib.rs"
        self.source.parent.mkdir(parents=True)
        self.source_bytes = b"pub fn fixture() -> u8 { 42 }\n"
        self.source.write_bytes(self.source_bytes)
        self.binary = self.root / "pathkeep-agent-history"
        self.binary_bytes = b"synthetic-agent-history-binary\x00version-one"
        self.binary.write_bytes(self.binary_bytes)
        self.repo = patch.object(verify, "REPO", self.root)
        self.repo.start()
        self.addCleanup(self.repo.stop)

    def test_source_archive_reports_unknown_git_provenance_and_real_digests(self):
        self.assertFalse((self.root / ".git").exists())
        result = verify.build_fingerprint(self.binary)

        self.assertEqual(result["git_provenance"], "unavailable")
        self.assertTrue(result["source_archive"])
        self.assertIsNone(result["git_head"])
        self.assertIsNone(result["git_dirty"])
        self.assertIsNone(result["git_status_sha256"])
        expected_source = hashlib.sha256(
            b"src-tauri/crates/agent-history/src/lib.rs" + self.source_bytes
        ).hexdigest()
        self.assertEqual(result["source_tree_sha256"], expected_source)
        self.assertEqual(result["binary_sha256"], hashlib.sha256(self.binary_bytes).hexdigest())
        self.assertEqual(result["binary_bytes"], len(self.binary_bytes))

    def test_archive_digests_are_repeatable_and_track_source_and_binary_independently(self):
        before = verify.build_fingerprint(self.binary)
        self.assertEqual(verify.build_fingerprint(self.binary), before)

        self.source.write_bytes(b"pub fn fixture() -> u8 { 43 }\n")
        source_changed = verify.build_fingerprint(self.binary)
        self.assertNotEqual(source_changed["source_tree_sha256"], before["source_tree_sha256"])
        self.assertEqual(source_changed["binary_sha256"], before["binary_sha256"])

        self.binary.write_bytes(b"synthetic-agent-history-binary\x00version-two")
        binary_changed = verify.build_fingerprint(self.binary)
        self.assertEqual(binary_changed["source_tree_sha256"], source_changed["source_tree_sha256"])
        self.assertNotEqual(binary_changed["binary_sha256"], source_changed["binary_sha256"])


class SyntheticSearchCoverageTests(unittest.TestCase):
    """A searchable tail alone cannot prove that all middle events were indexed."""

    def test_exact_native_event_total_and_single_tail_are_accepted(self):
        verify.require_synthetic_search_coverage(
            7, {"total": 1, "complete": True}, {"total": 7, "complete": True}
        )

    def test_missing_middle_event_rejects_the_scale_result(self):
        with self.assertRaisesRegex(verify.VerificationFailure, "synthetic_unicode_not_searchable"):
            verify.require_synthetic_search_coverage(
                7, {"total": 1, "complete": True}, {"total": 6, "complete": True}
            )

    def test_duplicated_tail_rejects_the_scale_result(self):
        with self.assertRaisesRegex(verify.VerificationFailure, "synthetic_tail_not_searchable"):
            verify.require_synthetic_search_coverage(
                7, {"total": 2, "complete": True}, {"total": 7, "complete": True}
            )

    def test_matching_count_with_incomplete_unicode_coverage_is_rejected(self):
        with self.assertRaisesRegex(verify.VerificationFailure, "synthetic_unicode_not_searchable"):
            verify.require_synthetic_search_coverage(
                7, {"total": 1, "complete": True}, {"total": 7, "complete": False}
            )


if __name__ == "__main__":
    unittest.main()
