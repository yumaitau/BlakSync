"""Command-line interface for the BlakSync org overlay."""

from __future__ import annotations

import argparse
import json
import sys

from .errors import BlakSyncError, RoleError
from .overlay import BlakSyncOrg, default_config_dir

HELP = """BlakSync org overlay — profile, roles, access notes, and audit

Each config directory is one organisation. Two config directories cannot see
each other's folders. There is no social login. Government ID is not stored.

The organisation writes each folder's access note. BlakSync does not invent
ceremony rules.

Commands:
  init              Create the local org profile (owner)
  org               Show the org profile
  org-set           Update name, timezone, or contact
  roles             List members and roles
  role-add          Add a member (owner only)
  role-set          Change a member's role (owner only)
  folders           List folders and access notes
  folder-add        Add a folder overlay (label + access note)
  folder-note       Set a folder's access note
  pending           Show pending devices with access notes
  pending-add       Record a pending device in this config directory
  accept            Accept a pending device (owner or admin)
  deny              Deny a pending device (owner or admin)
  share             Share a folder with an accepted device
  unshare           Stop sharing a folder with a device
  audit             Show the local audit log
  audit-export      Write the audit log as CSV (labels only, no paths)

Global options:
  --config-dir DIR  Config directory (or BLAKSYNC_CONFIG_DIR)
  --actor ID        Member id to act as (default: local owner from init)
  --json            Print JSON instead of text
"""


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    parser = _build_parser()
    if not argv or argv[0] in ("-h", "--help", "help"):
        print(HELP)
        return 0
    try:
        args = parser.parse_args(argv)
    except SystemExit as exc:
        return int(exc.code or 0)
    if not args.command:
        print(HELP)
        return 0
    org = BlakSyncOrg(args.config_dir)
    try:
        result = _dispatch(org, args)
    except RoleError as exc:
        if args.json:
            _print_error(exc, json_mode=True)
            return 1
        if exc.prompt is not None:
            sys.stdout.write(exc.prompt.text)
            if not exc.prompt.text.endswith("\n"):
                sys.stdout.write("\n")
            sys.stdout.write("\n")
        print(str(exc))
        return 1
    except BlakSyncError as exc:
        _print_error(exc, json_mode=args.json)
        return 1
    except ValueError as exc:
        _print_error(exc, json_mode=args.json)
        return 1
    _print_result(result, args)
    return 0


def _build_parser():
    parser = argparse.ArgumentParser(
        prog="blaksync",
        add_help=False,
        description="BlakSync org overlay",
    )
    parser.add_argument("--config-dir", default=default_config_dir())
    parser.add_argument("--actor", default=None)
    parser.add_argument("--json", action="store_true")
    sub = parser.add_subparsers(dest="command")

    init = sub.add_parser("init", add_help=False)
    init.add_argument("--name", required=True)
    init.add_argument("--timezone", required=True)
    init.add_argument("--contact", required=True)
    init.add_argument("--actor-id", default="owner")
    init.add_argument("--actor-name", default="Owner")

    sub.add_parser("org", add_help=False)

    org_set = sub.add_parser("org-set", add_help=False)
    org_set.add_argument("--name")
    org_set.add_argument("--timezone")
    org_set.add_argument("--contact")

    sub.add_parser("roles", add_help=False)

    role_add = sub.add_parser("role-add", add_help=False)
    role_add.add_argument("--id", dest="member_id", required=True)
    role_add.add_argument("--name", required=True)
    role_add.add_argument("--role", required=True)
    role_add.add_argument("--device")

    role_set = sub.add_parser("role-set", add_help=False)
    role_set.add_argument("--id", dest="member_id", required=True)
    role_set.add_argument("--role", required=True)

    sub.add_parser("folders", add_help=False)

    folder_add = sub.add_parser("folder-add", add_help=False)
    folder_add.add_argument("--id", dest="folder_id", required=True)
    folder_add.add_argument("--label", required=True)
    folder_add.add_argument("--note", default="")
    folder_add.add_argument("--who-may-pair", default=None)
    folder_add.add_argument("--path", default=None)

    folder_note = sub.add_parser("folder-note", add_help=False)
    folder_note.add_argument("--id", dest="folder_id", required=True)
    folder_note.add_argument("--note", required=True)

    sub.add_parser("pending", add_help=False)

    pending_add = sub.add_parser("pending-add", add_help=False)
    pending_add.add_argument("--device", required=True)
    pending_add.add_argument("--name", default="")
    pending_add.add_argument("--folder", action="append", default=[])

    accept = sub.add_parser("accept", add_help=False)
    accept.add_argument("--device", required=True)

    deny = sub.add_parser("deny", add_help=False)
    deny.add_argument("--device", required=True)

    share = sub.add_parser("share", add_help=False)
    share.add_argument("--folder", required=True)
    share.add_argument("--device", required=True)

    unshare = sub.add_parser("unshare", add_help=False)
    unshare.add_argument("--folder", required=True)
    unshare.add_argument("--device", required=True)

    sub.add_parser("audit", add_help=False)

    audit_export = sub.add_parser("audit-export", add_help=False)
    audit_export.add_argument("--out")

    return parser


def _dispatch(org: BlakSyncOrg, args):
    command = args.command
    actor = args.actor
    if command == "init":
        return {
            "kind": "org",
            "data": org.init(
                name=args.name,
                timezone=args.timezone,
                contact=args.contact,
                actor_id=args.actor_id,
                actor_name=args.actor_name,
            ),
        }
    if command == "org":
        return {"kind": "org", "data": org.get_org()}
    if command == "org-set":
        if args.name is None and args.timezone is None and args.contact is None:
            raise BlakSyncError("Pass --name, --timezone, and/or --contact.")
        return {
            "kind": "org",
            "data": org.set_org(
                actor_id=actor,
                name=args.name,
                timezone=args.timezone,
                contact=args.contact,
            ),
        }
    if command == "roles":
        return {"kind": "members", "data": org.list_members()}
    if command == "role-add":
        return {
            "kind": "member",
            "data": org.add_member(
                member_id=args.member_id,
                name=args.name,
                role=args.role,
                device_id=args.device,
                actor_id=actor,
            ),
        }
    if command == "role-set":
        return {
            "kind": "member",
            "data": org.set_role(args.member_id, args.role, actor_id=actor),
        }
    if command == "folders":
        return {"kind": "folders", "data": org.list_folders()}
    if command == "folder-add":
        return {
            "kind": "folder",
            "data": org.add_folder(
                folder_id=args.folder_id,
                label=args.label,
                access_note=args.note,
                who_may_pair=args.who_may_pair,
                path=args.path,
                actor_id=actor,
            ),
        }
    if command == "folder-note":
        return {
            "kind": "folder",
            "data": org.set_access_note(args.folder_id, args.note, actor_id=actor),
        }
    if command == "pending":
        return {"kind": "pending", "data": org.list_pending(actor_id=actor)}
    if command == "pending-add":
        return {
            "kind": "pending_device",
            "data": org.add_pending_device(
                device_id=args.device,
                name=args.name,
                folder_ids=args.folder,
                actor_id=actor,
            ),
        }
    if command == "accept":
        result = org.accept_device(args.device, actor_id=actor)
        return {"kind": "accept", "data": result}
    if command == "deny":
        return {"kind": "deny", "data": org.deny_device(args.device, actor_id=actor)}
    if command == "share":
        return {
            "kind": "folder",
            "data": org.share_folder(args.folder, args.device, actor_id=actor),
        }
    if command == "unshare":
        return {
            "kind": "folder",
            "data": org.unshare_folder(args.folder, args.device, actor_id=actor),
        }
    if command == "audit":
        return {"kind": "audit", "data": org.list_audit(actor_id=actor)}
    if command == "audit-export":
        csv_text = org.export_audit_csv(actor_id=actor)
        if args.out:
            with open(args.out, "w", encoding="utf-8", newline="") as handle:
                handle.write(csv_text)
            return {"kind": "audit_export", "data": {"path": args.out, "csv": csv_text}}
        return {"kind": "csv", "data": csv_text}
    raise BlakSyncError(f"Unknown command: {command}")


def _print_error(exc, *, json_mode):
    if json_mode:
        payload = {"error": str(exc)}
        if isinstance(exc, RoleError) and exc.prompt is not None:
            payload["prompt"] = exc.prompt.to_dict()
        print(json.dumps(payload, indent=2, ensure_ascii=False))
        return
    print(str(exc), file=sys.stderr)


def _print_result(result, args):
    kind = result["kind"]
    data = result["data"]
    if args.json:
        if kind == "csv":
            print(json.dumps({"csv": data}, ensure_ascii=False))
        else:
            print(json.dumps(data, indent=2, ensure_ascii=False))
        return
    if kind == "org":
        print(f"Organisation: {data['name']}")
        print(f"Timezone: {data['timezone']}")
        print(f"Contact: {data['contact']}")
        return
    if kind == "members":
        for member in data:
            device = f"  device {member['device_id']}" if member.get("device_id") else ""
            print(f"{member['id']}  {member['name']}  {member['role']}{device}")
        return
    if kind == "member":
        print(f"{data['id']}  {data['name']}  {data['role']}")
        return
    if kind == "folders":
        if not data:
            print("No folders in this organisation.")
            return
        for folder in data:
            print(folder["label"])
            print(f"  id: {folder['id']}")
            print(f"  organisation: {folder['org_name']}")
            print(f"  who may pair: {', '.join(folder['who_may_pair'])}")
            note = folder["access_note"].strip() or "(No access note has been written yet.)"
            print(f"  access note: {note}")
            if folder["shared_with"]:
                print(f"  shared with: {', '.join(folder['shared_with'])}")
            print()
        return
    if kind == "folder":
        print(data["label"])
        print(f"  id: {data['id']}")
        print(f"  organisation: {data['org_name']}")
        print(f"  who may pair: {', '.join(data['who_may_pair'])}")
        note = data["access_note"].strip() or "(No access note has been written yet.)"
        print(f"  access note: {note}")
        return
    if kind == "pending":
        if not data:
            print("No pending devices.")
            return
        blocks = [item["text"].rstrip() for item in data]
        print("\n\n".join(blocks))
        return
    if kind == "pending_device":
        print(f"Pending device recorded: {data['device_id']}")
        return
    if kind == "accept":
        sys.stdout.write(data["prompt"]["text"])
        print()
        print(f"Device accepted: {data['device_id']}")
        return
    if kind == "deny":
        print(f"Device denied: {data['device_id']}")
        return
    if kind == "audit":
        if not data:
            print("No audit events.")
            return
        for row in data:
            label = row.get("folder_label") or "-"
            device = row.get("device_id") or "-"
            print(
                f"{row['timestamp']}  {row['event']}  actor={row['actor']} "
                f"role={row['role']}  device={device}  folder={label}"
            )
        return
    if kind == "csv":
        sys.stdout.write(data)
        if not data.endswith("\n"):
            sys.stdout.write("\n")
        return
    if kind == "audit_export":
        print(f"Wrote audit CSV to {data['path']}")
        return
    print(json.dumps(data, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    raise SystemExit(main())
