#!/usr/bin/env python3
"""Serves a folder as the release pages `allot update` reads, on loopback, for the update tests.

    serve-release.py <folder> <port> <latest tag>

The folder holds download/<tag>/<file>. `GET /releases/latest` answers a redirect to
/releases/tag/<latest tag>, as github.com does, and every other path under /releases/ is a
file of the folder, so ALLOT_UPDATE_BASE=http://127.0.0.1:<port>/releases reaches it.
"""

import functools
import http.server
import sys

PREFIX = "/releases"


class Handler(http.server.SimpleHTTPRequestHandler):
    latest = ""

    def send_head(self):
        if self.path == f"{PREFIX}/latest":
            self.send_response(302)
            self.send_header("Location", f"{PREFIX}/tag/{self.latest}")
            self.send_header("Content-Length", "0")
            self.end_headers()
            return None
        if not self.path.startswith(f"{PREFIX}/"):
            self.send_error(404)
            return None
        self.path = self.path[len(PREFIX) :]
        return super().send_head()


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    folder, port, latest = sys.argv[1], int(sys.argv[2]), sys.argv[3]
    Handler.latest = latest
    handler = functools.partial(Handler, directory=folder)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), handler)
    server.serve_forever()


if __name__ == "__main__":
    main()
