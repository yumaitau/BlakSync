#!/usr/bin/env node
import { SyncthingClient } from "../lib/syncthing.js";

const [command, ...args] = process.argv.slice(2);
if (!command || command === "--help") usage(command ? 0 : 1);

try {
  const options = parseOptions(args);
  if (options.help) usage(0);
  const client = new SyncthingClient({ baseUrl: process.env.BLAKSYNC_URL, apiKey: process.env.BLAKSYNC_API_KEY });
  let result;
  switch (command) {
    case "device-id": result = { deviceId: await client.getDeviceId() }; break;
    case "pending-devices": result = await client.listPendingDevices(); break;
    case "devices": result = await client.listDevices(); break;
    case "add-device": result = await client.addDevice(required(options, "device"), options.name); break;
    case "add-folder": result = await client.addFolder(required(options, "folder"), required(options, "path"), options.label); break;
    case "office-folder": result = await client.addOfficeFolder(required(options, "folder"), process.env.BLAKSYNC_ORG_ROOT, options.label); break;
    case "accept-folder": result = await client.addFolder(required(options, "folder"), required(options, "path"), options.label); await client.shareFolder(required(options, "folder"), required(options, "device")); break;
    case "share": await client.shareFolder(required(options, "folder"), required(options, "device")); result = { shared: true }; break;
    case "unshare": await client.unshareFolder(required(options, "folder"), required(options, "device")); result = { shared: false }; break;
    case "pause": await client.setFolderPaused(required(options, "folder"), true); result = { paused: true }; break;
    case "resume": await client.setFolderPaused(required(options, "folder"), false); result = { paused: false }; break;
    case "status": result = await client.listFolderStatus(); break;
    case "health": result = await client.getOfficeHealth(); break;
    default: throw new Error(`Unknown command: ${command}`);
  }
  if (result !== undefined) console.log(JSON.stringify(result, null, 2));
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}

function parseOptions(args) {
  const parsed = {};
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (!arg.startsWith("--")) throw new Error(`Unexpected argument: ${arg}`);
    const key = arg.slice(2);
    if (key === "help") parsed.help = true;
    else parsed[key] = args[++index];
  }
  return parsed;
}
function required(options, name) { if (!options[name]) throw new Error(`Missing --${name}`); return options[name]; }
function usage(code) {
  console.log(`BlakSync — explicit Syncthing folder sharing

Set BLAKSYNC_API_KEY (required) and BLAKSYNC_URL (default http://127.0.0.1:8384).

Commands:
  device-id
  pending-devices
  devices
  add-device --device ID [--name NAME]
  add-folder --folder ID --path PATH [--label LABEL]
  office-folder --folder ID [--label LABEL]
  accept-folder --folder ID --path PATH --device ID [--label LABEL]
  share --folder ID --device ID
  unshare --folder ID --device ID
  pause --folder ID
  resume --folder ID
  status
  health`);
  process.exit(code);
}
