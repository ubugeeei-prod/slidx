import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import test from "node:test";

import { pack } from "../pack-npm.mjs";
import { argumentsOf, publish, registryVersion, tarballIntegrity } from "../publish-npm.mjs";

function registry(status, versions = {}) {
  return async () => ({
    status,
    ok: status >= 200 && status < 300,
    json: async () => ({ versions }),
  });
}

void test("a tarball list is resolved beside the list file", async (context) => {
  const directory = await mkdtemp(join(tmpdir(), "slidx-npm-list-"));
  context.after(() => rm(directory, { recursive: true, force: true }));
  const list = join(directory, "publish-order.txt");
  await writeFile(list, "one.tgz\ntwo.tgz\n");

  assert.deepEqual(argumentsOf(["--provenance", "--list", list]), {
    dryRun: false,
    provenance: true,
    tarballs: [join(directory, "one.tgz"), join(directory, "two.tgz")],
  });
});

void test("the registry version decides whether publication is already complete", async () => {
  assert.equal(await registryVersion("@slidxjs/audience", "0.6.0", registry(404)), undefined);
  assert.deepEqual(
    await registryVersion("@slidxjs/audience", "0.6.0", registry(200, { "0.6.0": {} })),
    {},
  );
  assert.equal(
    await registryVersion("@slidxjs/audience", "0.6.0", registry(200, { "0.5.0": {} })),
    undefined,
  );
});

void test("a rerun skips an existing version and publishes only a missing one", async (context) => {
  const directory = await mkdtemp(join(tmpdir(), "slidx-npm-publish-"));
  context.after(() => rm(directory, { recursive: true, force: true }));
  const tarball = pack("packages/audience", directory);
  const calls = [];
  const run = (command, args, options) => {
    calls.push({ command, args, options });
    return { status: 0, stdout: "", stderr: "" };
  };

  await publish(
    { dryRun: false, provenance: true, tarballs: [tarball] },
    registry(200, { "0.6.0": { dist: { integrity: tarballIntegrity(tarball) } } }),
    run,
  );
  assert.deepEqual(calls, []);

  await assert.rejects(
    publish(
      { dryRun: false, provenance: true, tarballs: [tarball] },
      registry(200, { "0.6.0": { dist: { integrity: "sha512-someone-elses-tarball" } } }),
      run,
    ),
    /exists with a different tarball/,
  );
  assert.deepEqual(calls, []);

  await publish({ dryRun: false, provenance: true, tarballs: [tarball] }, registry(404), run);
  assert.deepEqual(calls, [
    {
      command: "npm",
      args: ["publish", tarball, "--access", "public", "--provenance"],
      options: { encoding: "utf8" },
    },
  ]);
  assert.match(basename(tarball), /^slidxjs-audience-0\.6\.0\.tgz$/);
});
