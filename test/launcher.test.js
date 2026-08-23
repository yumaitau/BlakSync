import assert from "node:assert/strict";
import { afterEach, test } from "node:test";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { launchSyncthing } from "../lib/launcher.js";

const DEVICE_ID = "AAAAAAA-BBBBBBB-CCCCCCC-DDDDDDD-EEEEEEE-FFFFFFF-GGGGGGG-HHHHHHH";
const homes = [];

afterEach(async () => {
  await Promise.all(homes.splice(0).map((home) => rm(home, { recursive: true, force: true })));
});

test("generates a dedicated home, prints identity, and starts on loopback", async () => {
  const home = await temporaryHome();
  const commands = [];
  const logs = [];
  let serveArgs;
  const result = await launchSyncthing({
    binary: "/opt/syncthing",
    home,
    runCommand: async (binary, args) => {
      commands.push({ binary, args });
      return args[0] === "device-id" ? `${DEVICE_ID}\n` : "";
    },
    serve: async (binary, args, env) => {
      serveArgs = { binary, args, env };
      return 0;
    },
    logger: (message) => logs.push(message),
  });

  assert.deepEqual(commands.map(({ args }) => args[0]), ["generate", "device-id"]);
  assert.equal(commands[0].args.includes(`--home=${home}`), true);
  assert.deepEqual(serveArgs.args, [
    "serve",
    `--home=${home}`,
    "--gui-address=127.0.0.1:8384",
    "--no-browser",
    "--no-port-probing",
  ]);
  assert.equal(serveArgs.env.STVERSIONEXTRA, "BlakSync");
  assert.equal(logs.includes(`Device ID: ${DEVICE_ID}`), true);
  assert.equal(result.guiUrl, "http://127.0.0.1:8384");
});

test("keeps an existing Syncthing configuration", async () => {
  const home = await temporaryHome();
  await writeFile(path.join(home, "config.xml"), "<configuration />\n", "utf8");
  const commands = [];

  await launchSyncthing({
    home,
    runCommand: async (_binary, args) => {
      commands.push(args);
      return `${DEVICE_ID}\n`;
    },
    serve: async () => 0,
    logger: () => {},
  });

  assert.deepEqual(commands.map((args) => args[0]), ["device-id"]);
});

async function temporaryHome() {
  const home = await mkdtemp(path.join(tmpdir(), "blaksync-launcher-"));
  homes.push(home);
  return home;
}
