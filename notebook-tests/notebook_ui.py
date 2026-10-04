"""JupyterLab interactions for the DQL notebook tests."""

from __future__ import annotations

import re

from playwright.sync_api import Page, expect

from lab_server import LabServer


def open_lab(page: Page, lab: LabServer) -> None:
    page.goto(lab.lab_url)
    expect(page.locator(".jp-LabShell, #jupyterlab")).to_be_visible()


def open_smoke_notebook(page: Page, lab: LabServer) -> None:
    open_lab(page, lab)
    item = page.locator(".jp-DirListing-item").filter(has_text="smoke-dqlrs.ipynb")
    expect(item).to_be_visible()
    item.dblclick()
    expect(page.locator(".jp-Notebook")).to_be_visible()
    _accept_kernel_dialog(page, "DQL (Rust)")
    expect(page.locator(".jp-Notebook .jp-CodeCell").first).to_be_visible()


def _accept_kernel_dialog(page: Page, kernel_name: str) -> None:
    dialog = page.locator(".jp-Dialog")
    try:
        dialog.wait_for(state="visible", timeout=8_000)
    except Exception:
        return
    header = dialog.locator(".jp-Dialog-header")
    title = header.inner_text() if header.count() else ""
    if "kernel" not in title.lower():
        dismiss = dialog.locator(".jp-Dialog-close-button, .jp-mod-reject")
        if dismiss.count():
            dismiss.first.click()
        return
    select = dialog.locator("select")
    if select.count():
        labels = select.locator("option").all_inner_texts()
        match = next((label for label in labels if kernel_name in label), None)
        if match is None:
            raise AssertionError(f"{kernel_name} is not in the kernel list: {labels}")
        select.select_option(label=match.strip())
    else:
        choice = dialog.get_by_text(kernel_name, exact=False)
        if choice.count():
            choice.first.click()
    accept = dialog.locator(".jp-mod-accept")
    expect(accept).to_be_enabled()
    accept.click()
    expect(dialog).to_be_hidden()


def run_code_cell(page: Page, index: int) -> str:
    """Run a code cell and return its output text."""
    cell = page.locator(".jp-Notebook .jp-CodeCell").nth(index)
    expect(cell).to_be_visible()
    cell.locator(".cm-content").click()
    page.keyboard.press("Shift+Enter")
    prompt = cell.locator(".jp-InputPrompt")
    expect(prompt).to_have_text(re.compile(r"\[\d+\]"), timeout=90_000)
    output = cell.locator(".jp-OutputArea-output")
    if output.count() == 0:
        return ""
    return output.inner_text()
