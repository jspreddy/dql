"""Local web UI for DQL query files and DynamoDB tables."""

from __future__ import annotations

import argparse
import json
import os
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import parse_qs, urlparse

from engine import PAGE_SIZE, Engine, EngineError, find_dqlrs
from files import (
    PathError,
    create_file,
    delete_path,
    duplicate_path,
    make_dir,
    move_path,
    read_text,
    tree,
    write_text,
)

STATIC = Path(__file__).resolve().parent / "static"
ROOT: Path
ENGINE: Engine


class App(BaseHTTPRequestHandler):
    server_version = "DQLRSWeb/0.1"

    def do_GET(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        if parsed.path == "/api/config":
            self._json(
                {
                    "endpoint": ENGINE.endpoint_label,
                    "root": str(ROOT),
                    "page_size": PAGE_SIZE,
                }
            )
            return
        if parsed.path == "/api/tree":
            self._json(tree(ROOT))
            return
        if parsed.path == "/api/file":
            relative = parse_qs(parsed.query).get("path", [""])[0]
            try:
                self._json({"path": relative, "text": read_text(ROOT, relative)})
            except PathError as exc:
                self._error(400, str(exc))
            return
        if parsed.path == "/api/tables":
            pattern = parse_qs(parsed.query).get("pattern", [""])[0]
            try:
                self._json({"tables": ENGINE.list_tables(pattern)})
            except EngineError as exc:
                self._error(502, str(exc))
            return
        if parsed.path == "/api/rows":
            query = parse_qs(parsed.query)
            name = query.get("table", [""])[0]
            try:
                page = int(query.get("page", ["0"])[0])
            except ValueError:
                page = 0
            try:
                self._json(ENGINE.table_rows(name, page))
            except EngineError as exc:
                self._error(502, str(exc))
            return
        self._static(parsed.path)

    def do_POST(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        body = self._body()
        if parsed.path == "/api/file":
            try:
                path = create_file(ROOT, str(body.get("path") or ""))
            except PathError as exc:
                self._error(400, str(exc))
                return
            self._json({"path": path, "text": ""})
            return
        if parsed.path == "/api/mkdir":
            try:
                path = make_dir(ROOT, str(body.get("path") or ""))
            except PathError as exc:
                self._error(400, str(exc))
                return
            self._json({"path": path, "kind": "dir"})
            return
        if parsed.path == "/api/duplicate":
            try:
                path, kind = duplicate_path(ROOT, str(body.get("path") or ""))
            except PathError as exc:
                self._error(400, str(exc))
                return
            self._json({"path": path, "kind": kind})
            return
        if parsed.path == "/api/move":
            try:
                path, kind = move_path(ROOT, str(body.get("from") or ""), str(body.get("to") or ""))
            except PathError as exc:
                self._error(400, str(exc))
                return
            self._json({"path": path, "kind": kind})
            return
        if parsed.path == "/api/delete":
            try:
                delete_path(ROOT, str(body.get("path") or ""))
            except PathError as exc:
                self._error(400, str(exc))
                return
            self._json({"ok": True})
            return
        if parsed.path == "/api/run":
            try:
                results = ENGINE.run_script(str(body.get("dql") or ""))
            except EngineError as exc:
                self._error(502, str(exc))
                return
            ok = all(item.get("ok") for item in results)
            self._json({"ok": ok, "results": results})
            return
        self._error(404, "not found")

    def do_PUT(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        if parsed.path != "/api/file":
            self._error(404, "not found")
            return
        body = self._body()
        try:
            write_text(ROOT, str(body.get("path") or ""), str(body.get("text") or ""))
        except PathError as exc:
            self._error(400, str(exc))
            return
        self._json({"ok": True})

    def log_message(self, fmt: str, *args) -> None:
        print(f"[dqlrs-web] {self.address_string()} {fmt % args}")

    def _body(self) -> dict:
        length = int(self.headers.get("Content-Length") or 0)
        if length > 2_000_000:
            return {}
        raw = self.rfile.read(length) if length else b""
        if not raw:
            return {}
        try:
            value = json.loads(raw.decode("utf-8"))
        except json.JSONDecodeError:
            return {}
        return value if isinstance(value, dict) else {}

    def _json(self, payload: dict, status: int = 200) -> None:
        data = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def _error(self, status: int, message: str) -> None:
        self._json({"ok": False, "error": message}, status)

    def _static(self, path: str) -> None:
        relative = "index.html" if path in {"", "/"} else path.lstrip("/")
        file_path = (STATIC / relative).resolve()
        if STATIC.resolve() not in file_path.parents and file_path != STATIC.resolve():
            self._error(404, "not found")
            return
        if not file_path.is_file():
            self._error(404, "not found")
            return
        kind = {
            ".html": "text/html; charset=utf-8",
            ".css": "text/css; charset=utf-8",
            ".js": "text/javascript; charset=utf-8",
        }.get(file_path.suffix, "application/octet-stream")
        data = file_path.read_bytes()
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)


def main(argv: list[str] | None = None) -> None:
    global ROOT, ENGINE
    parser = argparse.ArgumentParser(description="DQLRS Web")
    parser.add_argument("--dir", default=os.getcwd(), help="Folder of .dql files")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--aws", action="store_true", help="Use AWS instead of DynamoDB Local")
    args = parser.parse_args(argv)
    ROOT = Path(args.dir).resolve()
    ROOT.mkdir(parents=True, exist_ok=True)
    if args.aws:
        host, port = "", "8000"
    else:
        host = os.environ.get("DQL_HOST", "localhost")
        port = os.environ.get("DQL_PORT", "8000")
    region = os.environ.get("AWS_REGION", "us-west-1")
    os.environ.setdefault("AWS_ACCESS_KEY_ID", "fakeid")
    os.environ.setdefault("AWS_SECRET_ACCESS_KEY", "fakekey")
    os.environ.setdefault("AWS_EC2_METADATA_DISABLED", "true")
    ENGINE = Engine(find_dqlrs(), host, port, region)
    httpd = ThreadingHTTPServer((args.host, args.port), App)
    print(f"DQLRS Web  http://{args.host}:{args.port}")
    print(f"workspace  {ROOT}")
    print(f"endpoint   {ENGINE.endpoint_label}")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print()
    finally:
        ENGINE.close()
        httpd.server_close()


if __name__ == "__main__":
    main()
