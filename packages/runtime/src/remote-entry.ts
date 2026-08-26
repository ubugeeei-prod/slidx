/**
 * The entry a pairing page downloads, and no audience slide does.
 *
 * Remote constructors used to live only on the package barrel, which no
 * shipped page imported. This file is what a presenter and a phone ask
 * for, and `readRemoteRuntime()` emits it whole — so the list is the names
 * those two pages write into their `import { … }`, and nothing else.
 */

export { pairingUrl, readPairing } from "./remote";
export { connectRelay, joinRemote, relaySocketUrl, rememberPairing } from "./remote-link";
export { renderQrSvg } from "./qr";
