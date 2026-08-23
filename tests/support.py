"""Shared fixtures for overlay tests."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from blaksync.overlay import BlakSyncOrg

ROOT = Path(__file__).resolve().parent.parent
PEER_A = "AAAAAAA-BBBBBBB-CCCCCCC-DDDDDDD-EEEEEEE-FFFFFFF-GGGGGGG-HHHHHHH"
PEER_B = "IIIIIII-JJJJJJJ-KKKKKKK-LLLLLLL-MMMMMMM-NNNNNNN-OOOOOOO-PPPPPPP"

ACCESS_NOTE = (
    "Speak with the cultural officer before pairing a new device. "
    "This collection is restricted."
)


class OverlayTestCase(unittest.TestCase):
    def setUp(self):
        self._tmpdir = tempfile.TemporaryDirectory()
        self.config_dir = Path(self._tmpdir.name) / "org-a"
        self.org = BlakSyncOrg(self.config_dir)
        self.org.init(
            name="Example Land Council",
            timezone="Australia/Darwin",
            contact="it@example.org.au",
            actor_id="office-node",
            actor_name="Office node",
        )

    def tearDown(self):
        self._tmpdir.cleanup()

    def add_member(self, member_id, role, name=None):
        return self.org.add_member(
            member_id=member_id,
            name=name or member_id,
            role=role,
            actor_id="office-node",
        )

    def add_heritage_folder(self, path="/var/lib/blaksync/secret-heritage"):
        return self.org.add_folder(
            folder_id="heritage-scans",
            label="Heritage scans",
            access_note=ACCESS_NOTE,
            path=path,
            actor_id="office-node",
        )

    def record_pending(self, device_id=PEER_A, folder_ids=None, name="Field tablet"):
        return self.org.add_pending_device(
            device_id=device_id,
            name=name,
            folder_ids=folder_ids if folder_ids is not None else ["heritage-scans"],
            actor_id="office-node",
        )

    def run_cli(self, *args, config_dir=None, actor=None):
        command = [sys.executable, "-m", "blaksync", "--config-dir", str(config_dir or self.config_dir)]
        if actor:
            command.extend(["--actor", actor])
        command.extend(args)
        env = os.environ.copy()
        env["PYTHONPATH"] = str(ROOT) + os.pathsep + env.get("PYTHONPATH", "")
        return subprocess.run(
            command,
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
