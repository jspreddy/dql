"""Drive JupyterLab and run DQL cells through the notebook UI."""

from __future__ import annotations

import pytest
from playwright.sync_api import expect

from notebook_ui import open_lab, open_smoke_notebook, run_code_cell


def test_launcher_lists_dql_kernels(lab_page, lab) -> None:
    open_lab(lab_page, lab)
    launcher = lab_page.locator(".jp-Launcher")
    expect(launcher).to_be_visible()
    # Notebook and Console sections each list the kernel.
    for name in ("DQL (Rust)", "DQL (Python)", "Python (dql)"):
        expect(launcher.get_by_text(name, exact=True).first).to_be_visible()


@pytest.mark.slow
def test_rust_kernel_runs_select(lab_page, lab) -> None:
    open_smoke_notebook(lab_page, lab)

    created = run_code_cell(lab_page, 0)
    assert "Created table 'pw_nb_smoke'" in created, created

    inserted = run_code_cell(lab_page, 1)
    assert "1 affected" in inserted, inserted
    progress = lab_page.locator(".jp-CodeCell").nth(1).locator("progress")
    if progress.count():
        assert progress.first.get_attribute("value") is not None, inserted

    selected = run_code_cell(lab_page, 2)
    assert "pw-ok" in selected, selected
    assert lab_page.locator(".jp-CodeCell").nth(2).locator("progress").count() == 0
