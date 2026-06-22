#!/usr/bin/env bash
# Serve web/ over HTTPS on all interfaces (reachable over WireGuard; satisfies
# WebGPU's secure-context requirement). Serves pre-gzipped .gz files with
# Content-Encoding: gzip when present (big win for the large .wasm on slow links).
# Self-signed cert in tools/certs.
#   ./tools/serve_https.sh [port]
set -euo pipefail
cd "$(dirname "$0")/.."
PORT="${1:-8443}"
exec python3 - "$PORT" <<'PY'
import http.server, ssl, sys, os, mimetypes

port = int(sys.argv[1])
os.chdir("web")

class H(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        path = self.translate_path(self.path)
        gz = path + ".gz"
        accepts = "gzip" in self.headers.get("Accept-Encoding", "")
        if accepts and os.path.isfile(gz) and os.path.isfile(path):
            ctype = mimetypes.guess_type(path)[0] or "application/octet-stream"
            if path.endswith(".wasm"):
                ctype = "application/wasm"
            data = open(gz, "rb").read()
            self.send_response(200)
            self.send_header("Content-Type", ctype)
            self.send_header("Content-Encoding", "gzip")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
            return
        super().do_GET()

srv = http.server.ThreadingHTTPServer(("0.0.0.0", port), H)
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain("../tools/certs/cert.pem", "../tools/certs/key.pem")
srv.socket = ctx.wrap_socket(srv.socket, server_side=True)
print(f"HTTPS+gzip serving web/ on 0.0.0.0:{port}", flush=True)
srv.serve_forever()
PY
