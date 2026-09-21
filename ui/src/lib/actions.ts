/** Focus a modal dialog when it opens (unless a field inside already grabbed focus), so Escape works. */
export function dialogFocus(node: HTMLElement) {
  queueMicrotask(() => {
    if (!node.contains(document.activeElement)) node.focus({ preventScroll: true })
  })
}
