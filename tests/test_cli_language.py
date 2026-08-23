"""CLI copy is Australian English and has no emojis."""

from __future__ import annotations

import re
import unittest

from blaksync.cli import HELP, main


class CliLanguageTests(unittest.TestCase):
    def test_help_uses_australian_english(self):
        self.assertIn("organisation", HELP.lower())
        self.assertNotIn("organization", HELP.lower())
        self.assertIn("access note", HELP.lower())
        self.assertIn("government id is not stored", HELP.lower())
        self.assertIn("no social login", HELP.lower())

    def test_help_has_no_emojis(self):
        self.assertIsNone(re.search(r"[\U0001F300-\U0001FAFF]", HELP))

    def test_main_help_exit_zero(self):
        from io import StringIO
        from unittest.mock import patch

        with patch("sys.stdout", new=StringIO()) as stdout:
            self.assertEqual(main(["--help"]), 0)
            self.assertIn("organisation", stdout.getvalue().lower())
