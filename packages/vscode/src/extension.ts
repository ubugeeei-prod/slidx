/**
 * The extension itself: find the binary, start the client, stop it on the way
 * out.
 *
 * Deliberately thin, for the same reason `slidx_cli`'s `main.rs` is. Everything
 * that decides anything is in `binary.ts` and `server.ts`, which import nothing
 * from `vscode` and are therefore testable without an editor. This file owns
 * the two things a test cannot hold: the extension host's lifecycle, and the
 * process the client library spawns.
 */

import { workspace, window, type ExtensionContext } from "vscode";
import {
  LanguageClient,
  TransportKind,
  type LanguageClientOptions,
  type ServerOptions,
} from "vscode-languageclient/node";

import { isFound, machine, nowhere, resolve } from "./binary";
import { CLIENT_ID, CLIENT_NAME, documentSelector, serverCommand } from "./server";

let client: LanguageClient | undefined;

export function activate(context: ExtensionContext): void {
  const configured = workspace.getConfiguration(CLIENT_ID).get<string>("path");
  const found = resolve(machine(configured));

  if (!isFound(found)) {
    // Said once, in words that name the places that were tried. A language
    // server that never starts is indistinguishable from one with nothing to
    // say, so silence here is the one thing this must not do.
    void window.showErrorMessage(nowhere(found));
    return;
  }

  const { command, args } = serverCommand(found.command);
  const server: ServerOptions = {
    run: { command, args: [...args], transport: TransportKind.stdio },
    // The same process either way. slidx has no debug build of the server to
    // switch to, and a `debug` entry that ran something else would be a second
    // answer nobody asked for.
    debug: { command, args: [...args], transport: TransportKind.stdio },
  };

  const options: LanguageClientOptions = {
    documentSelector: documentSelector().map((filter) => ({ ...filter })),
    // The server publishes findings for the document it was sent and reads
    // nothing off disk, so there is nothing on the filesystem for the client to
    // watch on its behalf.
  };

  client = new LanguageClient(CLIENT_ID, CLIENT_NAME, server, options);
  context.subscriptions.push(client);

  void client.start();
}

export function deactivate(): Promise<void> | undefined {
  return client?.stop();
}
