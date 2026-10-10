"""Session JupyterLab and Playwright defaults."""

from __future__ import annotations

import tempfile
from collections.abc import Iterator
from pathlib import Path

import pytest
from playwright.sync_api import Page

from lab_server import LabServer, start_lab


@pytest.fixture(scope="session")
def lab() -> Iterator[LabServer]:
    workdir = Path(tempfile.mkdtemp(prefix="dql-notebook-tests-"))
    server = start_lab(workdir)
    try:
        yield server
    finally:
        server.stop()


@pytest.fixture
def lab_page(page: Page) -> Page:
    page.set_viewport_size({"width": 1440, "height": 900})
    page.set_default_timeout(30_000)
    page.set_default_navigation_timeout(60_000)
    return page
