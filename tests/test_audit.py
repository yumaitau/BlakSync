"""Audit log: accepted, shared, unshared. CSV has labels, not paths or contents."""

from __future__ import annotations

import csv
import io

from tests.support import PEER_A, OverlayTestCase

SECRET_PATH = "/var/lib/blaksync/secret-heritage"
FILE_NAME = "restricted-scan-001.tif"


class AuditTests(OverlayTestCase):
    def setUp(self):
        super().setUp()
        self.add_heritage_folder(path=SECRET_PATH)
        self.record_pending()
        self.org.accept_device(PEER_A, actor_id="office-node")
        self.org.share_folder("heritage-scans", PEER_A, actor_id="office-node")
        self.org.unshare_folder("heritage-scans", PEER_A, actor_id="office-node")

    def test_events_and_timestamps(self):
        rows = self.org.list_audit(actor_id="office-node")
        events = [row["event"] for row in rows]
        self.assertEqual(events, ["device_accepted", "folder_shared", "folder_unshared"])
        for row in rows:
            self.assertTrue(row["timestamp"])
            self.assertIn("+", row["timestamp"][10:] or "+")
            self.assertEqual(row["actor"], "office-node")
            self.assertEqual(row["role"], "owner")
            self.assertEqual(set(row), {
                "timestamp",
                "event",
                "actor",
                "role",
                "device_id",
                "folder_label",
            })

    def test_csv_has_folder_labels_and_no_paths(self):
        csv_text = self.org.export_audit_csv(actor_id="office-node")
        self.assertIn("timestamp,event,actor,role,device_id,folder_label", csv_text)
        self.assertIn("Heritage scans", csv_text)
        self.assertIn("device_accepted", csv_text)
        self.assertIn("folder_shared", csv_text)
        self.assertIn("folder_unshared", csv_text)
        self.assertNotIn(SECRET_PATH, csv_text)
        self.assertNotIn("secret-heritage", csv_text)
        self.assertNotIn(FILE_NAME, csv_text)
        self.assertNotIn("path", csv_text.splitlines()[0])
        self.assertNotIn("access_note", csv_text.splitlines()[0])

        parsed = list(csv.DictReader(io.StringIO(csv_text)))
        self.assertEqual(len(parsed), 3)
        shared = parsed[1]
        self.assertEqual(shared["folder_label"], "Heritage scans")
        self.assertEqual(shared["device_id"], PEER_A)
        self.assertEqual(shared["event"], "folder_shared")
        self.assertNotIn("path", shared)

    def test_member_cannot_export_audit(self):
        from blaksync.errors import RoleError

        self.add_member("field-worker", "member")
        with self.assertRaises(RoleError):
            self.org.export_audit_csv(actor_id="field-worker")

    def test_cli_audit_export(self):
        result = self.run_cli("audit-export")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("folder_label", result.stdout)
        self.assertIn("Heritage scans", result.stdout)
        self.assertNotIn(SECRET_PATH, result.stdout)
