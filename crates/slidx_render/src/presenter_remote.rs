//! Pairing UI on the presenter page.
//!
//! The constructors live in `@slidxjs/runtime/remote`. This file is the
//! construction site: mint a pairing, draw its QR, and open the relay
//! without replacing the same-machine channel. A deck that never opted
//! in never includes this script, so it ships no additional bytes.

/// The import a presenter page adds when a relay was named.
pub(crate) fn import(remote_src: &str) -> String {
    format!(
        r#"import {{
  rememberPairing,
  pairingUrl,
  connectRelay,
  relaySocketUrl,
  joinRemote,
  renderQrSvg,
}} from "{remote_src}";
"#
    )
}

/// Opens the pairing, or falls back to the local channel alone.
///
/// The endpoint arrives on the document from the plugin, not from this
/// file, so a presenter rendered in a unit test — which has no Worker —
/// still constructs a mirror and still has no `https://` in it.
pub(crate) const BOOT: &str = r#"
const remoteConfig = document.documentElement.getAttribute("data-slidx-remote");
const remotePanel = document.querySelector("[data-slidx-remote-panel]");
const remoteToggle = document.querySelector('[data-slidx-action="remote"]');

function openRemoteMirror() {
  if (!remoteConfig) return createMirror();

  let config;
  try { config = JSON.parse(remoteConfig); } catch { return createMirror(); }
  if (!config.endpoint) return createMirror();

  const pairing = rememberPairing(browserStorage);
  const page = new URL("remote/", new URL(root, location.href));
  const url = pairingUrl(page.href, pairing);
  const qr = remotePanel?.querySelector("[data-slidx-remote-qr]");
  const link = remotePanel?.querySelector("[data-slidx-remote-url]");
  const svg = renderQrSvg(url);
  if (qr) qr.innerHTML = svg ?? "";
  if (link) {
    link.textContent = "Open on this phone";
    link.setAttribute("href", url);
  }

  const socket = connectRelay(relaySocketUrl(config.endpoint, pairing.session));
  return joinRemote({ pairing, socket, local: true });
}

const mirror = openRemoteMirror();

if (remoteToggle && remotePanel) {
  const showRemote = (open) => {
    remotePanel.hidden = !open;
    remoteToggle.setAttribute("aria-expanded", String(open));
  };
  remoteToggle.addEventListener("click", () => showRemote(remotePanel.hidden));
}
"#;

/// The control and the panel, omitted entirely when remote is off.
pub(crate) fn chrome() -> &'static str {
    r#"      <button
        type="button"
        data-slidx-action="remote"
        aria-expanded="false"
        aria-controls="slidx-remote"
      >
        Phone
      </button>
"#
}

pub(crate) fn panel() -> &'static str {
    r#"  <section
    class="slidx-remote"
    id="slidx-remote"
    data-slidx-remote-panel
    aria-label="Phone remote"
    hidden
  >
    <p class="slidx-remote-copy">Scan to drive the deck. The secret stays in this window.</p>
    <figure class="slidx-remote-qr" data-slidx-remote-qr></figure>
    <a class="slidx-remote-url" data-slidx-remote-url></a>
  </section>
"#
}
