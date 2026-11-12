// Confirm and prompt dialogs. Each factory builds its <dialog> (with the ids the page and its
// tests use), appends it to the body, and returns a function that resolves with the answer.
// Escape, Cancel, and a click on the backdrop answer "no".

function element(tag, props = {}, children = []) {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (key === "class") el.className = value;
    else if (key === "hidden") el.hidden = value;
    else el.setAttribute(key, value);
  }
  el.append(...children);
  return el;
}

/**
 * `confirm(message, { title, path, okLabel, danger })` resolves true for OK and false
 * otherwise. Asking again while a confirm is open answers the open one false.
 */
export function createConfirmDialog(parent = document.body) {
  const title = element("h2", { id: "confirm-pop-title" }, ["Confirm"]);
  const path = element("p", { id: "confirm-pop-path", class: "confirm-pop-path", hidden: true });
  const message = element("p", { id: "confirm-pop-message" });
  const cancel = element("button", { type: "button", id: "confirm-cancel" }, ["Cancel"]);
  const ok = element("button", { type: "button", id: "confirm-ok" }, ["OK"]);
  const dialog = element(
    "dialog",
    { id: "confirm-pop", "aria-labelledby": "confirm-pop-title", "aria-describedby": "confirm-pop-message" },
    [element("div", { class: "confirm-pop-card" }, [title, path, message, element("div", { class: "confirm-pop-actions" }, [cancel, ok])])],
  );
  parent.append(dialog);
  let pending = null;

  function settle(accepted) {
    const resolve = pending;
    pending = null;
    if (dialog.open) dialog.close();
    if (resolve) resolve(accepted);
  }

  ok.addEventListener("click", () => settle(true));
  cancel.addEventListener("click", () => settle(false));
  dialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    settle(false);
  });
  dialog.addEventListener("click", (event) => {
    if (event.target === dialog) settle(false);
  });

  return function confirm(text, { title: heading = "Confirm", path: shown = "", okLabel = "OK", danger = false } = {}) {
    if (pending) settle(false);
    title.textContent = heading;
    path.textContent = shown;
    path.hidden = !shown;
    message.textContent = text;
    ok.textContent = okLabel;
    ok.classList.toggle("danger", danger);
    return new Promise((resolve) => {
      pending = resolve;
      dialog.showModal();
      cancel.focus();
    });
  };
}

/**
 * `prompt(message, defaultValue, { okLabel })` resolves with the entered text, or null when
 * cancelled. The default value starts selected.
 */
export function createPromptDialog(parent = document.body) {
  const title = element("h2", { id: "prompt-pop-title" }, ["Name"]);
  const input = element("input", { id: "prompt-pop-input", type: "text", autocomplete: "off", spellcheck: "false" });
  const cancel = element("button", { type: "button", id: "prompt-cancel" }, ["Cancel"]);
  const ok = element("button", { type: "submit", id: "prompt-ok" }, ["OK"]);
  const form = element("form", { class: "prompt-pop-card", id: "prompt-pop-form" }, [
    title,
    input,
    element("div", { class: "prompt-pop-actions" }, [cancel, ok]),
  ]);
  const dialog = element("dialog", { id: "prompt-pop", "aria-labelledby": "prompt-pop-title" }, [form]);
  parent.append(dialog);
  let pending = null;

  function settle(value) {
    const resolve = pending;
    pending = null;
    if (dialog.open) dialog.close();
    if (resolve) resolve(value);
  }

  form.addEventListener("submit", (event) => {
    event.preventDefault();
    settle(input.value);
  });
  cancel.addEventListener("click", () => settle(null));
  dialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    settle(null);
  });
  dialog.addEventListener("click", (event) => {
    if (event.target === dialog) settle(null);
  });

  return function prompt(text, defaultValue = "", { okLabel = "OK" } = {}) {
    if (pending) settle(null);
    title.textContent = text;
    input.value = defaultValue == null ? "" : String(defaultValue);
    ok.textContent = okLabel;
    return new Promise((resolve) => {
      pending = resolve;
      dialog.showModal();
      input.focus();
      input.select();
    });
  };
}
