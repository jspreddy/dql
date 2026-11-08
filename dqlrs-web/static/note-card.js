// Shared note card. Query result notes and the structure dialog use the same card.

const VARIANTS = {
  error: "error",
  success: "check_circle",
  alert: "warning",
  info: "info",
};

export function noteCard(text, variant) {
  const icon = VARIANTS[variant];
  if (!icon) throw new Error("unknown note card variant: " + variant);
  const card = document.createElement("div");
  card.className = "note-card";
  card.dataset.variant = variant;
  card.dataset.testid = "note-card";
  const mark = document.createElement("span");
  mark.className = "note-card-icon";
  mark.setAttribute("aria-hidden", "true");
  mark.append(noteIcon(icon));
  const body = document.createElement("p");
  body.className = "note-card-text";
  body.textContent = text;
  card.append(mark, body);
  return card;
}

function noteIcon(name) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", "icon");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", "16");
  svg.setAttribute("height", "16");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS("http://www.w3.org/2000/svg", "use");
  use.setAttribute("href", `/vendor/icons/${name}.svg#icon`);
  svg.append(use);
  return svg;
}
