//! A static server for a deck that is already built.
//!
//! Enough HTTP to hand a browser the files `vite build` wrote, and nothing
//! else. It is not a dev server: it does not watch, does not reload, does not
//! transform, and never touches the source. It serves `dist/` exactly as a
//! static host would, which is the point — what you look at here is what the
//! artifact does.
//!
//! ## Three things it has to get right
//!
//! **`.js` has to arrive as JavaScript.** A slide with more than one stop
//! imports `./runtime.js` as a module, and a browser refuses a module served
//! with the wrong content type — with a console error about MIME checking that
//! says nothing about slides. The visible symptom is a staged deck frozen on
//! its first stop, which looks like a bug in the deck.
//!
//! **A directory has to resolve to its `index.html`.** The build writes
//! `slides/2/index.html`, and the link that reaches it is `/slides/2/`.
//!
//! **Loopback only.** Bound to `127.0.0.1`, never `0.0.0.0`. A preview of an
//! unreleased talk should not be reachable from conference wifi, and binding
//! wide by default is how that happens without anybody deciding it.
//!
//! ## And one it must not get wrong
//!
//! No request may escape the directory being served. `..` is rejected before a
//! path is built and the result is checked against the root again afterwards —
//! two defences, because this is the one bug in a static server that turns a
//! preview into a way to read somebody's home directory.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

/// Enough for ordinary browser headers without letting a local client make the
/// preview process retain an unbounded line.
const MAX_REQUEST_HEADER_BYTES: usize = 64 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// What to serve, from where.
#[derive(Debug, Clone)]
pub struct Site {
    root: PathBuf,
}

impl Site {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The file a request target names, or `None` if there is not one.
    ///
    /// `None` covers a missing file and an attempt to leave the root alike:
    /// telling the two apart in a reply would confirm what does and does not
    /// exist outside the directory being served.
    pub fn resolve(&self, target: &str) -> Option<PathBuf> {
        let path = self.root.join(safe_path(target)?);

        let path = if path.is_dir() { path.join("index.html") } else { path };
        if !path.is_file() {
            return None;
        }

        // Checked again after the fact, because a symlink inside the directory
        // can point out of it and no amount of string handling would see that.
        let real = path.canonicalize().ok()?;
        let root = self.root.canonicalize().ok()?;

        real.starts_with(&root).then_some(real)
    }
}

/// The path part of a request target, as components that cannot escape.
///
/// Rejects rather than sanitises: a target containing `..` is not a request
/// anybody's browser makes for a deck, so quietly rewriting it into something
/// legal would hide the only interesting case.
fn safe_path(target: &str) -> Option<PathBuf> {
    let path = target.split(['?', '#']).next().unwrap_or("");
    let decoded = percent_decode(path);
    let mut safe = PathBuf::new();

    for component in Path::new(decoded.trim_start_matches('/')).components() {
        match component {
            Component::Normal(part) => safe.push(part),
            // A bare `/` and `./` are ordinary; anything that climbs is not.
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    Some(safe)
}

/// `%20` and friends, because a slide file can have a space in its name.
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&text[index + 1..index + 3], 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }

        out.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
}

/// The content type for a file, by extension.
///
/// `.js` is the one that matters and the one a naive table gets wrong: a module
/// served as anything but JavaScript is refused outright, and a staged deck
/// then sits on its first stop looking broken.
pub fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "gif" => "image/gif",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain; charset=utf-8",
        "wasm" => "application/wasm",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        _ => "application/octet-stream",
    }
}

/// The target from a request line — `GET /slides/2/ HTTP/1.1`.
pub fn request_target(line: &str) -> Option<&str> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?;

    // Only reads. A preview server that honoured anything else would be
    // answering requests nobody meant to make.
    (method == "GET" || method == "HEAD").then(|| parts.next()).flatten()
}

/// Binds a loopback port. `0` lets the operating system choose a free one.
pub fn bind(port: u16) -> std::io::Result<TcpListener> {
    // Explicitly the loopback address rather than a hostname: `localhost` can
    // resolve to something else entirely, and "unreleased talk on the
    // conference wifi" is not a failure worth risking for a preview.
    TcpListener::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)))
}

/// Serves until the process is stopped.
pub fn serve(listener: &TcpListener, site: &Site) {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };

        // One request per connection, and every reply says so. Keep-alive
        // would need a read loop and a timeout, and a preview server that
        // holds a browser's connection open is one that appears to hang.
        let _ = answer(&mut stream, site);
    }
}

fn answer(stream: &mut TcpStream, site: &Site) -> std::io::Result<()> {
    // A browser always finishes its headers immediately. The timeout keeps a
    // half-written local request from holding the single-threaded preview
    // server forever.
    stream.set_read_timeout(Some(REQUEST_TIMEOUT))?;
    let mut reader = BufReader::new(stream.try_clone()?);

    let Some(line) = read_request_line(&mut reader)? else {
        return reply(stream, 400, "text/plain; charset=utf-8", b"bad request");
    };
    let Some(target) = request_target(&line) else {
        return reply(stream, 400, "text/plain; charset=utf-8", b"bad request");
    };

    match site.resolve(target) {
        Some(path) => {
            let body = fs::read(&path)?;
            reply(stream, 200, content_type(&path), &body)
        }
        None => reply(stream, 404, "text/plain; charset=utf-8", b"not found"),
    }
}

/// Reads the request line and consumes the remaining headers.
///
/// Leaving `Host` and the other header bytes unread when the server closes the
/// socket makes Windows send a reset instead of a graceful end-of-stream. A
/// browser then receives a complete response and an error at once. Consuming
/// through the empty line is both the HTTP boundary and what lets every
/// platform close the same way.
fn read_request_line(reader: &mut impl BufRead) -> std::io::Result<Option<String>> {
    let mut line = String::new();
    let mut read = reader.read_line(&mut line)?;
    if read == 0 || read > MAX_REQUEST_HEADER_BYTES {
        return Ok(None);
    }

    loop {
        let mut header = String::new();
        let bytes = reader.read_line(&mut header)?;
        read = read.saturating_add(bytes);

        if read > MAX_REQUEST_HEADER_BYTES {
            return Ok(None);
        }
        if bytes == 0 || header == "\r\n" || header == "\n" {
            return Ok(Some(line));
        }
    }
}

fn reply(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "Not Found",
    };

    // No-store, because the whole point of looking at a preview is to look at
    // it again after rebuilding. A cached first stop would be indistinguishable
    // from a deck that did not change.
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n",
        body.len()
    )?;

    stream.write_all(body)?;
    stream.flush()?;

    // Do not rely on dropping the socket to tell the browser the response is
    // complete. Winsock may turn that implicit close into a reset, even after
    // every response byte arrived. An explicit send-half shutdown queues FIN
    // after the body while leaving the receive half available until the
    // connection handle itself is released.
    stream.shutdown(Shutdown::Write)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Built(PathBuf);

    impl Built {
        /// A directory laid out the way `vite build` leaves one.
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("slidx-serve-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);

            fs::create_dir_all(root.join("slides/2")).expect("scratch");
            fs::write(root.join("slides/index.html"), "<h1>one</h1>").expect("slide");
            fs::write(root.join("slides/2/index.html"), "<h1>two</h1>").expect("slide");
            fs::write(root.join("slides/runtime.js"), "export const runtime = 1;")
                .expect("runtime");

            Self(root)
        }

        fn site(&self) -> Site {
            Site::new(&self.0)
        }
    }

    impl Drop for Built {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_runtime_is_served_as_javascript_so_a_staged_deck_can_import_it() {
        // The failure this exists to prevent: a browser refuses a module with
        // the wrong content type, and the deck sits frozen on its first stop
        // looking like a bug in the deck.
        assert_eq!(content_type(Path::new("runtime.js")), "text/javascript; charset=utf-8");
        assert_eq!(content_type(Path::new("runtime.mjs")), "text/javascript; charset=utf-8");
    }

    #[test]
    fn a_directory_resolves_to_the_index_the_build_wrote_into_it() {
        // The build writes slides/2/index.html and the link that reaches it is
        // /slides/2/.
        let built = Built::new("index");
        let site = built.site();

        assert_eq!(site.resolve("/slides/2/"), site.resolve("/slides/2/index.html"));
        assert!(site.resolve("/slides/").is_some());
    }

    #[test]
    fn a_file_is_served_by_its_own_path() {
        let built = Built::new("file");

        assert!(built.site().resolve("/slides/runtime.js").is_some());
    }

    #[test]
    fn a_query_string_is_not_part_of_the_file_name() {
        // The runtime carries a deep link as `?slide=3`, and the browser sends
        // it back on the request.
        let built = Built::new("query");

        assert!(built.site().resolve("/slides/runtime.js?v=2").is_some());
        assert!(built.site().resolve("/slides/2/#stop-1").is_some());
    }

    #[test]
    fn a_percent_encoded_space_finds_the_file_it_names() {
        let built = Built::new("space");
        fs::write(built.0.join("slides/a file.svg"), "<svg/>").expect("write");

        assert!(built.site().resolve("/slides/a%20file.svg").is_some());
    }

    #[test]
    fn nothing_can_climb_out_of_the_directory_being_served() {
        // The one bug in a static server that turns a preview into a way to
        // read somebody's home directory.
        let built = Built::new("traversal");
        let site = built.site();

        for target in [
            "/../../../../etc/passwd",
            "/slides/../../../etc/passwd",
            "/slides/..%2f..%2f..%2fetc%2fpasswd",
            "//etc/passwd",
            "/./../etc/passwd",
        ] {
            assert!(site.resolve(target).is_none(), "{target} escaped");
        }
    }

    #[test]
    fn an_absolute_windows_style_target_does_not_escape_either() {
        let built = Built::new("absolute");

        assert!(built.site().resolve("/C:/Windows/System32/drivers/etc/hosts").is_none());
    }

    #[test]
    fn a_missing_file_and_an_escape_attempt_are_answered_the_same_way() {
        // Telling them apart would confirm what does and does not exist
        // outside the directory being served.
        let built = Built::new("same");
        let site = built.site();

        assert!(site.resolve("/slides/nope.html").is_none());
        assert!(site.resolve("/../../etc/passwd").is_none());
    }

    #[test]
    fn a_current_directory_component_is_ordinary_rather_than_an_escape() {
        let built = Built::new("curdir");

        assert!(built.site().resolve("/./slides/runtime.js").is_some());
    }

    #[test]
    fn every_kind_of_file_a_deck_ships_has_a_content_type() {
        // A deck is HTML, one module, CSS, cards, fonts and whatever the
        // author embedded. `application/octet-stream` on any of these means a
        // browser refuses or downloads it.
        for (name, expected) in [
            ("index.html", "text/html; charset=utf-8"),
            ("style.css", "text/css; charset=utf-8"),
            ("og-1.svg", "image/svg+xml"),
            ("og-1.png", "image/png"),
            ("Inter.woff2", "font/woff2"),
            ("deck.pdf", "application/pdf"),
            ("clip.mp4", "video/mp4"),
        ] {
            assert_eq!(content_type(Path::new(name)), expected, "for {name}");
        }
    }

    #[test]
    fn something_with_no_extension_is_handed_over_as_bytes_rather_than_guessed_at() {
        assert_eq!(content_type(Path::new("LICENSE")), "application/octet-stream");
    }

    #[test]
    fn a_request_line_yields_its_target() {
        assert_eq!(request_target("GET /slides/2/ HTTP/1.1"), Some("/slides/2/"));
        assert_eq!(request_target("HEAD /runtime.js HTTP/1.1"), Some("/runtime.js"));
    }

    #[test]
    fn a_request_is_consumed_through_the_empty_line_after_its_headers() {
        use std::io::Cursor;

        let source =
            b"GET /slides/2/ HTTP/1.1\r\nHost: localhost\r\nAccept: text/html\r\n\r\nafter";
        let boundary = source.windows(4).position(|bytes| bytes == b"\r\n\r\n").unwrap() + 4;
        let mut request = Cursor::new(source);

        assert_eq!(
            read_request_line(&mut request).unwrap().as_deref(),
            Some("GET /slides/2/ HTTP/1.1\r\n")
        );
        assert_eq!(
            request.position() as usize,
            boundary,
            "the response reader reached into the body"
        );
    }

    #[test]
    fn an_unbounded_header_is_refused_before_a_response_is_written() {
        use std::io::Cursor;

        let request = format!(
            "GET / HTTP/1.1\r\nX-Too-Large: {}\r\n\r\n",
            "x".repeat(MAX_REQUEST_HEADER_BYTES)
        );

        assert!(read_request_line(&mut Cursor::new(request)).unwrap().is_none());
    }

    #[test]
    fn anything_but_a_read_is_refused() {
        // A preview server that honoured a POST would be answering requests
        // nobody meant to make.
        for line in ["POST / HTTP/1.1", "PUT /x HTTP/1.1", "DELETE / HTTP/1.1"] {
            assert_eq!(request_target(line), None, "{line}");
        }
    }

    #[test]
    fn a_malformed_request_line_is_not_a_target() {
        assert_eq!(request_target(""), None);
        assert_eq!(request_target("GET"), None);
    }

    #[test]
    fn the_listener_binds_loopback_and_nothing_else() {
        // Not 0.0.0.0. An unreleased talk should not be reachable from
        // conference wifi, and binding wide by default is how that happens
        // without anybody deciding it.
        let listener = bind(0).expect("bind");
        let address = listener.local_addr().expect("address");

        assert!(address.ip().is_loopback(), "{address}");
    }

    #[test]
    fn a_served_deck_answers_a_real_request_over_a_real_socket() {
        use std::io::Read;
        use std::net::TcpStream;

        let built = Built::new("live");
        let listener = bind(0).expect("bind");
        let port = listener.local_addr().expect("address").port();
        let site = built.site();

        let server = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = answer(&mut stream, &site);
            }
        });

        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        write!(stream, "GET /slides/runtime.js HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("write");

        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read");
        server.join().expect("server");

        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(response.contains("Content-Type: text/javascript"), "{response}");
        assert!(response.contains("export const runtime"), "{response}");
    }

    #[test]
    fn a_request_for_something_that_is_not_there_is_answered_rather_than_dropped() {
        use std::io::Read;
        use std::net::TcpStream;

        let built = Built::new("missing");
        let listener = bind(0).expect("bind");
        let port = listener.local_addr().expect("address").port();
        let site = built.site();

        let server = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let _ = answer(&mut stream, &site);
            }
        });

        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        write!(stream, "GET /nope HTTP/1.1\r\n\r\n").expect("write");

        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read");
        server.join().expect("server");

        assert!(response.starts_with("HTTP/1.1 404"), "{response}");
    }

    #[test]
    fn the_response_reaches_eof_before_the_server_drops_its_socket_handle() {
        use std::io::Read;
        use std::net::TcpStream;
        use std::sync::mpsc;

        let built = Built::new("finish");
        let listener = bind(0).expect("bind");
        let port = listener.local_addr().expect("address").port();
        let site = built.site();
        let (release_server, hold_server) = mpsc::channel();

        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            answer(&mut stream, &site).expect("answer");
            hold_server.recv().expect("release");
        });

        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        stream.set_read_timeout(Some(Duration::from_secs(2))).expect("timeout");
        write!(stream, "GET /slides/runtime.js HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("write");

        let mut response = String::new();
        let read = stream.read_to_string(&mut response);
        release_server.send(()).expect("release");
        server.join().expect("server");

        read.expect("the explicit send shutdown reaches the client as EOF");
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(response.contains("export const runtime"), "{response}");
    }
}
