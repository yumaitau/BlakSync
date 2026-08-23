"""Organisation profile: name, Australia/* timezone, contact. No government ID."""

from __future__ import annotations

from blaksync.errors import ConfigError, RoleError
from tests.support import OverlayTestCase


class OrgProfileTests(OverlayTestCase):
    def test_profile_round_trip(self):
        org = self.org.get_org()
        self.assertEqual(org["name"], "Example Land Council")
        self.assertEqual(org["timezone"], "Australia/Darwin")
        self.assertEqual(org["contact"], "it@example.org.au")
        self.assertNotIn("government_id", org)
        self.assertNotIn("path", org)

    def test_timezone_must_be_australian(self):
        with self.assertRaises(ConfigError) as caught:
            self.org.set_org(timezone="Pacific/Auckland", actor_id="office-node")
        self.assertIn("Australia/", str(caught.exception))

    def test_unknown_australia_timezone_is_rejected(self):
        with self.assertRaises(ConfigError):
            self.org.set_org(timezone="Australia/NotAPlace", actor_id="office-node")

    def test_sydney_timezone_is_accepted(self):
        updated = self.org.set_org(timezone="Australia/Sydney", actor_id="office-node")
        self.assertEqual(updated["timezone"], "Australia/Sydney")

    def test_government_id_is_not_stored(self):
        with self.assertRaises(ConfigError) as caught:
            self.org.set_org(government_id="123456789", actor_id="office-node")
        self.assertIn("Government ID", str(caught.exception))

    def test_init_rejects_medicare_field(self):
        from blaksync.overlay import BlakSyncOrg

        other = BlakSyncOrg(self.config_dir.parent / "org-b")
        with self.assertRaises(ConfigError) as caught:
            other.init(
                name="Second Org",
                timezone="Australia/Perth",
                contact="ops@example.org.au",
                medicare="1234567890",
            )
        self.assertIn("Government ID", str(caught.exception))

    def test_member_cannot_change_profile(self):
        self.add_member("field-worker", "member")
        with self.assertRaises(RoleError):
            self.org.set_org(contact="other@example.org.au", actor_id="field-worker")

    def test_admin_cannot_rename_organisation(self):
        self.add_member("it-admin", "admin")
        with self.assertRaises(RoleError):
            self.org.set_org(name="Renamed", actor_id="it-admin")

    def test_admin_can_update_contact(self):
        self.add_member("it-admin", "admin")
        updated = self.org.set_org(contact="admin@example.org.au", actor_id="it-admin")
        self.assertEqual(updated["contact"], "admin@example.org.au")
