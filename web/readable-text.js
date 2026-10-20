// Small safe presentation subset: all model content is text, never HTML.
function inline(node, text) {
  for (const part of text.split(/(\*\*[^*\n]+\*\*|`[^`\n]+`)/g)) {
    const tag = part.startsWith("**")
      ? "strong"
      : part.startsWith("`")
        ? "code"
        : null;
    if (tag) {
      const c = document.createElement(tag);
      c.textContent = part.slice(
        tag === "strong" ? 2 : 1,
        tag === "strong" ? -2 : -1,
      );
      node.append(c);
    } else node.append(document.createTextNode(part));
  }
}
export function readableText(node, text) {
  node.replaceChildren();
  let code = null,
    paragraph = [];
  const flush = () => {
    if (paragraph.length) {
      const p = document.createElement("p");
      inline(p, paragraph.join("\n"));
      node.append(p);
      paragraph = [];
    }
  };
  for (const line of String(text || "").split("\n")) {
    if (line.startsWith("```")) {
      flush();
      if (code) {
        const d = document.createElement("details"),
          s = document.createElement("summary"),
          p = document.createElement("pre");
        s.textContent = "代码";
        p.textContent = code.join("\n");
        d.append(s, p);
        node.append(d);
        code = null;
      } else code = [];
      continue;
    }
    if (code) {
      code.push(line);
      continue;
    }
    if (!line.trim()) {
      flush();
      continue;
    }
    if (/^#{1,6}\s/.test(line)) {
      flush();
      const h = document.createElement("h4");
      inline(h, line.replace(/^#+\s/, ""));
      node.append(h);
      continue;
    }
    paragraph.push(line);
  }
  flush();
  if (code) {
    const p = document.createElement("pre");
    p.textContent = code.join("\n");
    node.append(p);
  }
}
