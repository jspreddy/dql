"""JupyterLab interactions for the DQL notebook tests."""

from __future__ import annotations

import re

from playwright.sync_api import Page, expect

from lab_server import REPO_ROOT, LabServer

ARTIFACTS = REPO_ROOT / ".playwright-artifacts"


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


def _describe_stuck_cell(page: Page, lab: LabServer | None, index: int) -> str:
    """What the page and the server showed when a cell did not finish."""
    cell = page.locator(".jp-Notebook .jp-CodeCell").nth(index)
    lines = [f"cell {index} did not finish"]

    def text(label: str, locator) -> None:
        try:
            found = [value.strip() for value in locator.all_inner_texts() if value.strip()]
        except Exception as exc:  # noqa: BLE001 - diagnostics must not hide the failure
            found = [f"<{exc}>"]
        lines.append(f"{label}: {found}")

    text("prompt", cell.locator(".jp-InputPrompt"))
    text("output", cell.locator(".jp-OutputArea"))
    text("status bar", page.locator("#jp-main-statusbar"))
    text("kernel", page.locator(".jp-Toolbar-kernelName, .jp-KernelStatus"))
    text("dialog", page.locator(".jp-Dialog"))
    try:
        focused = page.evaluate("() => document.activeElement && document.activeElement.className")
    except Exception as exc:  # noqa: BLE001
        focused = f"<{exc}>"
    lines.append(f"focused element: {focused}")
    try:
        shot = ARTIFACTS / f"notebook-stuck-cell-{index}.png"
        ARTIFACTS.mkdir(parents=True, exist_ok=True)
        page.screenshot(path=str(shot))
        lines.append(f"screenshot: {shot}")
    except Exception as exc:  # noqa: BLE001
        lines.append(f"screenshot failed: {exc}")
    if lab is not None:
        try:
            log = lab.log_path.read_text(encoding="utf-8", errors="replace")
            lines.append("jupyter log tail:\n" + log[-3000:])
        except OSError as exc:
            lines.append(f"jupyter log: {exc}")
    return "\n".join(lines)


def run_code_cell(page: Page, index: int, lab: LabServer | None = None) -> str:
    """Run a code cell and return its output text."""
    cell = page.locator(".jp-Notebook .jp-CodeCell").nth(index)
    expect(cell).to_be_visible()
    cell.locator(".cm-content").click()
    page.keyboard.press("Shift+Enter")
    prompt = cell.locator(".jp-InputPrompt")
    try:
        expect(prompt).to_have_text(re.compile(r"\[\d+\]"), timeout=90_000)
    except AssertionError as exc:
        raise AssertionError(_describe_stuck_cell(page, lab, index)) from exc
    area = cell.locator(".jp-OutputArea")
    if area.count() == 0:
        return ""
    return area.first.inner_text()
