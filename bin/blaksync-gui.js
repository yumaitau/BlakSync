#!/usr/bin/env node
import { createGuiServer } from "../lib/gui-server.js";

const options = parseArgs(process.argv.slice(2));
if (options.help) {
  console.log(`BlakSync local web GUI

Binds to 127.0.0.1 by default. Talks to Syncthing through its REST API.
Syncthing's own GUI stays available as a fallback.

Environment:
  BLAKSYNC_API_KEY   Syncthing GUI API key (required)
  BLAKSYNC_URL       Syncthing GUI URL (default http://127.0.0.1:8384)
  BLAKSYNC_CONFIG_DIR  Access-note store (default ~/.config/blaksync)
  BLAKSYNC_GUI_HOST  Bind host (default 127.0.0.1)
  BLAKSYNC_GUI_PORT  Bind port (default 8385)

Options:
  --host HOST
  --port PORT
  --help`);
  process.exit(0);
}

if (!process.env.BLAKSYNC_API_KEY) {
  console.error("Set BLAKSYNC_API_KEY to your local Syncthing API key. The key is not printed.");
  process.exit(1);
}

const host = options.host || process.env.BLAKSYNC_GUI_HOST || "127.0.0.1";
const port = Number(options.port || process.env.BLAKSYNC_GUI_PORT || 8385);
const gui = createGuiServer({ host, port });

const url = await gui.listen();
console.log(`BlakSync GUI listening on ${url}`);
console.log("Syncthing stock GUI remains available as a fallback on its own address.");

function parseArgs(args) {
  const parsed = {};
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    if (arg === "--help" || arg === "-h") parsed.help = true;
    else if (arg === "--host") parsed.host = args[++index];
    else if (arg === "--port") parsed.port = args[++index];
    else throw new Error(`Unexpected argument: ${arg}`);
  }
  return parsed;
}
