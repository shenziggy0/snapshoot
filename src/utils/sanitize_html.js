const ALLOWED = new Set(['b', 'strong', 'i', 'em', 'u', 'br', 'span', 'p', 'div', 'mark', 'sub', 'sup']);
const DROP = new Set(['script', 'style', 'iframe', 'object', 'embed', 'link', 'meta']);

// Dictionary services return small HTML snippets (bold words, line breaks).
// Keep those tags and drop every attribute so a service cannot inject script.
export function sanitizeHtml(value) {
    const template = document.createElement('template');
    template.innerHTML = String(value ?? '');
    const walk = (node) => {
        for (const child of [...node.childNodes]) {
            if (child.nodeType !== Node.ELEMENT_NODE) {
                continue;
            }
            const tag = child.tagName.toLowerCase();
            if (DROP.has(tag) || !ALLOWED.has(tag)) {
                if (DROP.has(tag)) {
                    child.remove();
                } else {
                    child.replaceWith(document.createTextNode(child.textContent || ''));
                }
                continue;
            }
            while (child.attributes.length > 0) {
                child.removeAttribute(child.attributes[0].name);
            }
            walk(child);
        }
    };
    walk(template.content);
    return template.innerHTML;
}
