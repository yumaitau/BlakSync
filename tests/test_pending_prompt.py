"""Pending-device prompt shows the access note and folder list before Accept."""

from __future__ import annotations

from blaksync.errors import RoleError
from blaksync.roles import MEMBER_CANNOT_ACCEPT
from tests.support import ACCESS_NOTE, PEER_A, OverlayTestCase


class PendingPromptTests(OverlayTestCase):
    def setUp(self):
        super().setUp()
        self.add_heritage_folder(path="/var/lib/blaksync/secret-heritage")
        self.add_member("field-worker", "member")
        self.record_pending()

    def test_prompt_includes_access_note_and_folder_label(self):
        prompt = self.org.pending_prompt(PEER_A, actor_id="office-node")
        self.assertIn(ACCESS_NOTE, prompt.text)
        self.assertIn("Heritage scans", prompt.text)
        self.assertIn("Example Land Council", prompt.text)
        self.assertIn(PEER_A, prompt.text)
        self.assertTrue(prompt.can_accept)
        self.assertEqual(prompt.folders[0].access_note, ACCESS_NOTE)
        self.assertEqual(prompt.folders[0].label, "Heritage scans")

    def test_prompt_does_not_include_filesystem_path(self):
        prompt = self.org.pending_prompt(PEER_A, actor_id="office-node")
        self.assertNotIn("/var/lib/blaksync/secret-heritage", prompt.text)
        self.assertNotIn("secret-heritage", prompt.text)
        as_dict = prompt.to_dict()
        self.assertNotIn("path", as_dict)
        self.assertNotIn("path", as_dict["folders"][0])

    def test_member_sees_access_note_but_cannot_accept(self):
        prompt = self.org.pending_prompt(PEER_A, actor_id="field-worker")
        self.assertIn(ACCESS_NOTE, prompt.text)
        self.assertFalse(prompt.can_accept)
        self.assertEqual(prompt.refusal_reason, MEMBER_CANNOT_ACCEPT)
        self.assertIn("A member cannot accept a new device", prompt.text)

    def test_accept_prints_prompt_before_accepting(self):
        result = self.org.accept_device(PEER_A, actor_id="office-node")
        self.assertIn(ACCESS_NOTE, result["prompt"]["text"])
        self.assertTrue(result["accepted"])

    def test_cli_pending_shows_access_note_before_accept(self):
        listed = self.run_cli("pending")
        self.assertEqual(listed.returncode, 0, listed.stderr)
        self.assertIn(ACCESS_NOTE, listed.stdout)
        self.assertIn("Heritage scans", listed.stdout)
        self.assertIn("Read the access notes before you accept", listed.stdout)
        self.assertNotIn("/var/lib/blaksync/secret-heritage", listed.stdout)

        refused = self.run_cli("accept", "--device", PEER_A, actor="field-worker")
        self.assertNotEqual(refused.returncode, 0)
        combined = refused.stdout + refused.stderr
        self.assertIn(ACCESS_NOTE, combined)
        self.assertIn(MEMBER_CANNOT_ACCEPT, combined)

        accepted = self.run_cli("accept", "--device", PEER_A)
        self.assertEqual(accepted.returncode, 0, accepted.stderr)
        self.assertIn(ACCESS_NOTE, accepted.stdout)
        self.assertIn("Device accepted", accepted.stdout)

    def test_cli_json_accept_as_member_includes_prompt(self):
        refused = self.run_cli("--json", "accept", "--device", PEER_A, actor="field-worker")
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn(ACCESS_NOTE, refused.stdout)
        self.assertIn("can_accept", refused.stdout)
