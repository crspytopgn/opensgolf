// A local stand-in for the deployed Worker, for testing the game against it: node multiplayer/worker/test/serve.mjs [port]
// then run the game with OPENSGOLF_BOARD_URL=http://127.0.0.1:<port>. Results live in memory only.
import { createServer } from "node:http";
import { memoryDb } from "./d1shim.mjs";
import { memoryBucket } from "./r2shim.mjs";
import worker from "../src/index.js";

const env = { DB: memoryDb(), SAVES: memoryBucket(), ADMIN_PASSWORD: "local" };
const port = Number(process.argv[2] || 8787);
createServer(async (req, res) => {
  const chunks = [];
  for await (const c of req) chunks.push(c);
  const body = ["GET", "HEAD"].includes(req.method) ? undefined : Buffer.concat(chunks);
  const r = await worker.fetch(new Request(`http://127.0.0.1:${port}${req.url}`, { method: req.method, headers: req.headers, body }), env);
  const bytes = Buffer.from(await r.arrayBuffer());
  console.log(req.method, req.url.slice(0, 60), r.status, bytes.subarray(0, 200).toString());
  res.writeHead(r.status, Object.fromEntries(r.headers));
  res.end(bytes);
}).listen(port, "127.0.0.1", () => console.log(`board on http://127.0.0.1:${port}`));
