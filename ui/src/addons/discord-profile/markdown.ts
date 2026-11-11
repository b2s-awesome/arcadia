// The slice of Discord markdown that About me supports, rendered to safe
// HTML. Text is escaped first, so nothing typed can inject markup.

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

function inline(s: string): string {
  // Code first, so its contents aren't formatted.
  const codes: string[] = [];
  s = s.replace(/`([^`\n]+)`/g, (_, c) => `\u0000${codes.push(c) - 1}\u0000`);
  s = esc(s);
  s = s
    .replace(/\|\|(.+?)\|\|/g, '<span class="md-spoiler" tabindex="0">$1</span>')
    .replace(/\*\*\*(.+?)\*\*\*/g, "<strong><em>$1</em></strong>")
    .replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>")
    .replace(/__(.+?)__/g, "<u>$1</u>")
    .replace(/~~(.+?)~~/g, "<s>$1</s>")
    .replace(/(^|[^*\w])\*(?!\s)(.+?)\*(?!\w)/g, "$1<em>$2</em>")
    .replace(/(^|[^_\w])_(?!\s)(.+?)_(?!\w)/g, "$1<em>$2</em>")
    .replace(/(https?:\/\/[^\s<]+[^\s<.,:;!?)'"])/g, '<span class="md-link">$1</span>');
  return s.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${esc(codes[Number(i)])}</code>`);
}

export function renderBio(src: string): string {
  return src
    .split("\n")
    .map((line) => {
      const h = /^(#{1,3}) (.+)$/.exec(line);
      if (h) return `<span class="md-h${h[1].length}">${inline(h[2])}</span>`;
      const sub = /^-# (.+)$/.exec(line);
      if (sub) return `<span class="md-sub">${inline(sub[1])}</span>`;
      const q = /^> (.*)$/.exec(line);
      if (q) return `<span class="md-quote">${inline(q[1])}</span>`;
      const li = /^[-*] (.+)$/.exec(line);
      if (li) return `<span class="md-li">${inline(li[1])}</span>`;
      return inline(line);
    })
    .join("<br>");
}
