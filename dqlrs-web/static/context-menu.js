// A context menu of buttons and separators, placed at a point and kept inside the window.
// A click outside it or Escape closes it.

/**
 * `createContextMenu(menu, { ids, icon })` returns { show(x, y, items), hide(), element }.
 * `menu` is the menu element (created and appended to the body when omitted). Items are
 * `{ label, run, danger, icon }` or "-" for a separator. `ids[label]` names a button's id and
 * `icon(name)` builds the element for an item's `icon`.
 */
export function createContextMenu(menu, { ids = {}, icon = null } = {}) {
  let el = menu;
  if (!el) {
    el = document.createElement("div");
    el.className = "menu";
    el.setAttribute("role", "menu");
    el.hidden = true;
    document.body.append(el);
  }

  function hide() {
    el.hidden = true;
  }

  function show(x, y, items) {
    el.replaceChildren();
    for (const item of items) {
      if (item === "-") {
        el.append(document.createElement("hr"));
        continue;
      }
      const button = document.createElement("button");
      button.type = "button";
      button.role = "menuitem";
      if (ids[item.label]) button.id = ids[item.label];
      if (item.icon && icon) button.append(icon(item.icon));
      button.append(item.label);
      if (item.danger) button.className = "danger";
      button.addEventListener("click", (event) => {
        event.stopPropagation();
        hide();
        item.run();
      });
      el.append(button);
    }
    el.hidden = false;
    el.style.left = "0px";
    el.style.top = "0px";
    const rect = el.getBoundingClientRect();
    el.style.left = Math.max(4, Math.min(x, window.innerWidth - rect.width - 8)) + "px";
    el.style.top = Math.max(4, Math.min(y, window.innerHeight - rect.height - 8)) + "px";
  }

  document.addEventListener("click", (event) => {
    if (!el.contains(event.target)) hide();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") hide();
  });

  return { show, hide, element: el };
}
