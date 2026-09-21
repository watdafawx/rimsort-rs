const FOCUSABLE =
  'a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])'

/**
 * Modal dialog behaviour: focus it when it opens (unless a field inside already grabbed focus, so
 * Escape works) and keep Tab / Shift+Tab cycling inside it.
 */
export function dialogFocus(node: HTMLElement) {
  queueMicrotask(() => {
    if (!node.contains(document.activeElement)) node.focus({ preventScroll: true })
  })
  const onKey = (e: KeyboardEvent) => {
    if (e.key !== 'Tab') return
    const items = [...node.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
      (el) => el.offsetParent !== null || el === document.activeElement,
    )
    if (!items.length) return
    const first = items[0]
    const last = items[items.length - 1]
    const active = document.activeElement
    if (e.shiftKey && (active === first || active === node)) {
      e.preventDefault()
      last.focus()
    } else if (!e.shiftKey && active === last) {
      e.preventDefault()
      first.focus()
    }
  }
  node.addEventListener('keydown', onKey)
  return { destroy: () => node.removeEventListener('keydown', onKey) }
}
