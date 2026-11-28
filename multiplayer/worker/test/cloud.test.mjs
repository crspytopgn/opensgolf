import test from "node:test";
import assert from "node:assert/strict";
import { memoryDb } from "./d1shim.mjs";
import { memoryBucket } from "./r2shim.mjs";
import worker from "../src/index.js";
import { CLOUD_FILES, cloudName, isCloudCode } from "../src/cloud.js";

const CODE = "7K3QZ-M4P2A-XB9RT-0WV5C";

test("sync codes and file names", () => {
  assert.ok(isCloudCode(CODE));
  for (const bad of ["7k3qz-m4p2a-xb9rt-0wv5c", "7K3QZM4P2AXB9RT0WV5C", "7K3QZ-M4P2A-XB9RT-0WV5I", "", null]) assert.ok(!isCloudCode(bad), bad);
  for (const ok of ["Saved Games/My Club.sgs", "saved games/&Latest.sgs", "Saved Games/saves.json", "Championship/Pebble.sgch",
    "Championship/Ann.pro", "settings.json", "TOP10.SVE"]) assert.ok(cloudName(ok), ok);
  for (const bad of ["Saved Games/Backups/My Club~1.sgs", "snapshots/accomp3.png", "../settings.json", "Saved Games/x.exe",
    "packs/a.zip", "Saved Games/a\\b.sgs", "Saved Games/../settings.json"]) assert.ok(!cloudName(bad), bad);
});

test("files are kept, listed, read back, replaced and deleted by code", async () => {
  const env = { DB: memoryDb(), SAVES: memoryBucket() };
  const call = (path, init) => worker.fetch(new Request(`https://x.test${path}`, init), env);
  const name = encodeURIComponent("Saved Games/My Club.sgs");
  assert.deepEqual(await (await call(`/cloud/${CODE}`)).json(), { ok: true, files: [] });
  let r = await call(`/cloud/${CODE}/${name}?at=1000`, { method: "PUT", body: new Uint8Array([1, 2, 3]) });
  assert.equal(r.status, 200);
  r = await call(`/cloud/${CODE}/${encodeURIComponent("saved games/my club.sgs")}?at=2000`, { method: "PUT", body: new Uint8Array([4, 5]) });
  assert.equal(r.status, 200);
  const list = (await (await call(`/cloud/${CODE}`)).json()).files;
  assert.deepEqual(list, [{ name: "saved games/my club.sgs", size: 2, at: 2000 }]);
  r = await call(`/cloud/${CODE}/${name}`);
  assert.equal(r.headers.get("X-Saved-At"), "2000");
  assert.deepEqual(new Uint8Array(await r.arrayBuffer()), new Uint8Array([4, 5]));
  // another code sees none of them
  assert.deepEqual((await (await call("/cloud/AAAAA-BBBBB-CCCCC-DDDDD")).json()).files, []);
  assert.equal((await call(`/cloud/${CODE}/${name}`, { method: "DELETE" })).status, 200);
  assert.equal((await call(`/cloud/${CODE}/${name}`)).status, 404);
});

test("cloud saves refuse what they do not keep", async () => {
  const env = { DB: memoryDb(), SAVES: memoryBucket() };
  const call = (path, init) => worker.fetch(new Request(`https://x.test${path}`, init), env);
  assert.equal((await call("/cloud/nope")).status, 400);
  assert.equal((await call(`/cloud/${CODE}/${encodeURIComponent("packs/x.zip")}?at=1`, { method: "PUT", body: "x" })).status, 400);
  assert.equal((await call(`/cloud/${CODE}/settings.json?at=1`, { method: "PUT", body: new Uint8Array(0) })).status, 400);
  assert.equal((await call(`/cloud/${CODE}/settings.json`, { method: "PUT", body: "x" })).status, 400);
  for (let i = 0; i < CLOUD_FILES; i++) {
    const n = encodeURIComponent(`Saved Games/G${i}.sgs`);
    assert.equal((await call(`/cloud/${CODE}/${n}?at=1`, { method: "PUT", body: "x" })).status, 200);
  }
  const more = await call(`/cloud/${CODE}/${encodeURIComponent("Saved Games/One more.sgs")}?at=1`, { method: "PUT", body: "x" });
  assert.equal((await more.json()).error, "too many files");
  // replacing one is still fine
  assert.equal((await call(`/cloud/${CODE}/${encodeURIComponent("Saved Games/G0.sgs")}?at=2`, { method: "PUT", body: "y" })).status, 200);
  // a Worker without the bucket says so
  const bare = await worker.fetch(new Request(`https://x.test/cloud/${CODE}`), { DB: memoryDb() });
  assert.equal(bare.status, 404);
});
