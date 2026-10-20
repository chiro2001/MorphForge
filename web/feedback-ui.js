// Feedback must live in the active top-layer dialog, or outside hidden pages.
export function createFeedback(node, documentRoot = document) {
  let timer;
  function host() {
    return (
      documentRoot.activeElement?.closest("dialog[open]") ||
      [...documentRoot.querySelectorAll("dialog[open]")].at(-1) ||
      documentRoot.body
    );
  }
  documentRoot.body.append(node);
  documentRoot.addEventListener(
    "close",
    (event) => {
      if (node.parentElement === event.target) documentRoot.body.append(node);
    },
    true,
  );
  return (text, kind = "info") => {
    const parent = host();
    if (parent !== documentRoot.body) parent.prepend(node);
    else parent.append(node);
    node.textContent = String(text);
    node.dataset.kind = kind;
    node.setAttribute("role", kind === "error" ? "alert" : "status");
    node.classList.add("visible");
    clearTimeout(timer);
    timer = setTimeout(() => node.classList.remove("visible"), 4500);
  };
}
