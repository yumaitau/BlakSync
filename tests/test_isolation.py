"""Two organisations (two config directories) cannot see each other's folders."""

from __future__ import annotations

from pathlib import Path

from blaksync.errors import NotFoundError
from blaksync.overlay import BlakSyncOrg
from tests.support import ACCESS_NOTE, PEER_A, OverlayTestCase


class IsolationTests(OverlayTestCase):
    def setUp(self):
        super().setUp()
        self.add_heritage_folder()
        self.record_pending()

        self.other_dir = Path(self._tmpdir.name) / "org-b"
        self.other = BlakSyncOrg(self.other_dir)
        self.other.init(
            name="Second Community Corp",
            timezone="Australia/Perth",
            contact="ops@second.example.org.au",
            actor_id="perth-office",
            actor_name="Perth office",
        )
        self.other.add_folder(
            folder_id="payroll",
            label="Payroll",
            access_note="Staff only.",
        )

    def test_folders_are_not_visible_across_config_dirs(self):
        labels_a = {folder["label"] for folder in self.org.list_folders()}
        labels_b = {folder["label"] for folder in self.other.list_folders()}
        self.assertEqual(labels_a, {"Heritage scans"})
        self.assertEqual(labels_b, {"Payroll"})
        self.assertNotIn("Payroll", labels_a)
        self.assertNotIn("Heritage scans", labels_b)

    def test_pending_devices_are_not_visible_across_config_dirs(self):
        pending_a = self.org.list_pending()
        pending_b = self.other.list_pending()
        self.assertEqual(len(pending_a), 1)
        self.assertEqual(pending_a[0]["device_id"], PEER_A)
        self.assertEqual(pending_b, [])
        with self.assertRaises(NotFoundError):
            self.other.pending_prompt(PEER_A)

    def test_access_notes_do_not_leak_across_orgs(self):
        notes_b = [folder["access_note"] for folder in self.other.list_folders()]
        self.assertNotIn(ACCESS_NOTE, notes_b)

    def test_audit_logs_are_separate(self):
        self.org.accept_device(PEER_A)
        self.org.share_folder("heritage-scans", PEER_A)
        csv_a = self.org.export_audit_csv()
        csv_b = self.other.export_audit_csv()
        self.assertIn("device_accepted", csv_a)
        self.assertIn("Heritage scans", csv_a)
        self.assertIn("timestamp,event,actor,role,device_id,folder_label", csv_b)
        self.assertNotIn("device_accepted", csv_b)
        self.assertNotIn("Heritage scans", csv_b)

    def test_cli_with_two_config_dirs(self):
        listed_a = self.run_cli("folders")
        listed_b = self.run_cli("folders", config_dir=self.other_dir)
        self.assertIn("Heritage scans", listed_a.stdout)
        self.assertNotIn("Payroll", listed_a.stdout)
        self.assertIn("Payroll", listed_b.stdout)
        self.assertNotIn("Heritage scans", listed_b.stdout)
