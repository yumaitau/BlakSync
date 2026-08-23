"""Role checks: owner, admin, member. A member cannot accept a new device."""

from __future__ import annotations

from blaksync.errors import ConfigError, RoleError
from blaksync.roles import MEMBER_CANNOT_ACCEPT
from tests.support import ACCESS_NOTE, PEER_A, OverlayTestCase


class RoleTests(OverlayTestCase):
    def setUp(self):
        super().setUp()
        self.add_heritage_folder()
        self.add_member("it-admin", "admin", name="IT admin")
        self.add_member("field-worker", "member", name="Field worker")
        self.record_pending()

    def test_member_cannot_accept_new_device(self):
        with self.assertRaises(RoleError) as caught:
            self.org.accept_device(PEER_A, actor_id="field-worker")
        self.assertEqual(str(caught.exception), MEMBER_CANNOT_ACCEPT)
        self.assertIsNotNone(caught.exception.prompt)
        self.assertFalse(caught.exception.prompt.can_accept)
        pending = self.org.list_pending(actor_id="office-node")
        self.assertEqual(len(pending), 1)

    def test_admin_can_accept_new_device(self):
        result = self.org.accept_device(PEER_A, actor_id="it-admin")
        self.assertTrue(result["accepted"])
        self.assertEqual(self.org.list_pending(actor_id="it-admin"), [])

    def test_owner_can_accept_new_device(self):
        result = self.org.accept_device(PEER_A, actor_id="office-node")
        self.assertTrue(result["accepted"])

    def test_member_cannot_share_folder(self):
        self.org.accept_device(PEER_A, actor_id="office-node")
        with self.assertRaises(RoleError):
            self.org.share_folder("heritage-scans", PEER_A, actor_id="field-worker")

    def test_member_cannot_set_access_note(self):
        with self.assertRaises(RoleError):
            self.org.set_access_note("heritage-scans", "changed", actor_id="field-worker")

    def test_member_cannot_add_folder(self):
        with self.assertRaises(RoleError):
            self.org.add_folder(
                folder_id="work-docs",
                label="Work docs",
                actor_id="field-worker",
            )

    def test_member_can_list_accepted_folders(self):
        folders = self.org.list_folders()
        self.assertEqual(folders[0]["label"], "Heritage scans")
        self.assertEqual(folders[0]["access_note"], ACCESS_NOTE)

    def test_member_cannot_assign_roles(self):
        with self.assertRaises(RoleError):
            self.org.add_member(
                member_id="someone",
                name="Someone",
                role="member",
                actor_id="field-worker",
            )

    def test_admin_cannot_assign_roles(self):
        with self.assertRaises(RoleError) as caught:
            self.org.set_role("field-worker", "admin", actor_id="it-admin")
        self.assertIn("owner", str(caught.exception).lower())

    def test_owner_cannot_drop_the_last_owner(self):
        with self.assertRaises(ConfigError):
            self.org.set_role("office-node", "admin", actor_id="office-node")

    def test_who_may_pair_cannot_include_member(self):
        with self.assertRaises(ConfigError) as caught:
            self.org.add_folder(
                folder_id="photos",
                label="Photos",
                who_may_pair=["owner", "member"],
                actor_id="office-node",
            )
        self.assertIn("Members cannot pair", str(caught.exception))

    def test_admin_cannot_share_owner_only_folder(self):
        self.org.add_folder(
            folder_id="mens-business",
            label="Restricted collection",
            access_note="Owner pairing only.",
            who_may_pair=["owner"],
            actor_id="office-node",
        )
        self.org.accept_device(PEER_A, actor_id="office-node")
        with self.assertRaises(RoleError):
            self.org.share_folder("mens-business", PEER_A, actor_id="it-admin")
        self.org.share_folder("mens-business", PEER_A, actor_id="office-node")
        shared = [item for item in self.org.list_folders() if item["id"] == "mens-business"][0]
        self.assertEqual(shared["shared_with"], [PEER_A])
