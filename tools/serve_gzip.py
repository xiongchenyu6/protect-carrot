#!/usr/bin/env python3
"""HTTPS + gzip static server for web/.

- Serves pre-gzipped `.gz` siblings with `Content-Encoding: gzip` when the client
  asked for the plain file and accepts gzip.
- Adds `ETag` + `Cache-Control: no-cache` to every file and answers
  `If-None-Match` with `304 Not Modified` — so the browser revalidates cheaply and
  does NOT re-download the ~11MB wasm (or any asset) on every refresh; a new build
  changes the ETag and triggers exactly one fresh download.

Usage: python3 tools/serve_gzip.py [port]
"""
import http.server, ssl, sys, os, mimetypes

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8443
HERE = os.path.dirname(os.path.abspath(__file__))
os.chdir(os.path.join(HERE, "..", "web"))


class H(http.server.SimpleHTTPRequestHandler):
    def _etag(self, file):
        st = os.stat(file)
        return f'"{int(st.st_mtime)}-{st.st_size}"'

    def _serve(self, file, ctype, content_encoding=None):
        etag = self._etag(file)
        # Conditional request → 304, no body (saves the full download).
        if self.headers.get("If-None-Match") == etag:
            self.send_response(304)
            self.send_header("ETag", etag)
            self.send_header("Cache-Control", "no-cache")
            self.end_headers()
            return
        data = open(file, "rb").read()
        self.send_response(200)
        self.send_header("Content-Type", ctype)
        if content_encoding:
            self.send_header("Content-Encoding", content_encoding)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("ETag", etag)
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()
        if self.command == "GET":
            self.wfile.write(data)

    def _handle(self):
        path = self.translate_path(self.path)
        # 1) Transparent compression swap: client requested `foo`, we have a precompressed
        #    sibling. Prefer brotli (`foo.br`, ~35% smaller) when the client accepts it,
        #    else gzip. This mirrors what Cloudflare does in production.
        ae = self.headers.get("Accept-Encoding", "")
        br = path + ".br"
        gz = path + ".gz"
        if os.path.isfile(path):
            ctype = mimetypes.guess_type(path)[0] or "application/octet-stream"
            if path.endswith(".wasm"):
                ctype = "application/wasm"
            if "br" in ae and os.path.isfile(br):
                return self._serve(br, ctype, content_encoding="br")
            if "gzip" in ae and os.path.isfile(gz):
                return self._serve(gz, ctype, content_encoding="gzip")
        # 2) A directly-requested `.br` file: serve it WITH `Content-Encoding: br` so the
        #    browser decompresses it transparently. boot() fetches `…_bg.wasm.br` (a URL
        #    nothing special-cases) to pull the brotli engine — ~35% smaller than gzip.
        #    We tag it `br` UNCONDITIONALLY (not gated on Accept-Encoding): some clients
        #    (e.g. a wallet extension that rewrites fetch) drop `br` from Accept-Encoding
        #    even though the browser can decode it. Without the header the browser would
        #    receive raw brotli bytes, fail the wasm-magic check, and silently fall back
        #    to the larger gzip. boot() only ever requests this URL when it wants brotli,
        #    and still falls back to gzip if decoding fails — so tagging it is safe.
        if path.endswith(".br") and os.path.isfile(path):
            ctype = "application/wasm" if path.endswith(".wasm.br") else "application/octet-stream"
            return self._serve(path, ctype, content_encoding="br")
        # 3) Direct file (incl. the *.wasm.gz that boot() fetches + decompresses itself,
        #    which must be served RAW with no Content-Encoding). Cached via ETag.
        if os.path.isfile(path):
            ctype = mimetypes.guess_type(path)[0] or "application/octet-stream"
            if path.endswith(".wasm"):
                ctype = "application/wasm"
            elif path.endswith(".gz") or path.endswith(".br"):
                ctype = "application/octet-stream"
            return self._serve(path, ctype)
        # 3) Directories / 404s → default handler.
        return super().do_GET() if self.command == "GET" else super().do_HEAD()

    def do_GET(self):
        self._handle()

    def do_HEAD(self):
        self._handle()


srv = http.server.ThreadingHTTPServer(("0.0.0.0", PORT), H)
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain(os.path.join(HERE, "certs", "cert.pem"), os.path.join(HERE, "certs", "key.pem"))
srv.socket = ctx.wrap_socket(srv.socket, server_side=True)
print(f"HTTPS+gzip serving web/ on 0.0.0.0:{PORT}", flush=True)
srv.serve_forever()
