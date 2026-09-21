/**
 * Rich tooltip: `use:tip={{ title, text, keys }}`. One shared floating element for the whole app, shown
 * after a short hover delay, hidden on leave / press / scroll / Escape. Pure DOM, no per-element cost
 * beyond four listeners.
 */
import { t } from './i18n.svelte'

export type TipContent = {
  title: string
  /** One or two sentences on what it does / why it is disabled. */
  text?: string
  /** Shortcut(s), e.g. "Ctrl+S". */
  keys?: string
}

const DELAY = 380
let el: HTMLDivElement | null = null
let timer: ReturnType<typeof setTimeout> | undefined

function ensure(): HTMLDivElement {
  if (el) return el
  el = document.createElement('div')
  el.className = 'tip'
  el.setAttribute('role', 'tooltip')
  document.body.appendChild(el)
  return el
}

function hide() {
  clearTimeout(timer)
  el?.classList.remove('on')
}

function show(target: HTMLElement, c: TipContent) {
  const tip = ensure()
  tip.replaceChildren()
  const h = document.createElement('div')
  h.className = 'tip-title'
  h.textContent = t(c.title)
  tip.appendChild(h)
  if (c.text) {
    const p = document.createElement('div')
    p.className = 'tip-text'
    p.textContent = t(c.text)
    tip.appendChild(p)
  }
  if (c.keys) {
    const k = document.createElement('div')
    k.className = 'tip-keys'
    for (const part of c.keys.split('+')) {
      const kbd = document.createElement('kbd')
      kbd.textContent = part.trim()
      k.appendChild(kbd)
    }
    tip.appendChild(k)
  }
  // Measure while invisible, then place below the target (above if there is no room), clamped to the window.
  tip.style.left = '0px'
  tip.style.top = '0px'
  const r = target.getBoundingClientRect()
  const w = tip.offsetWidth
  const hgt = tip.offsetHeight
  const left = Math.max(8, Math.min(window.innerWidth - w - 8, r.left + r.width / 2 - w / 2))
  const below = r.bottom + 8
  const top = below + hgt > window.innerHeight - 8 ? Math.max(8, r.top - hgt - 8) : below
  tip.style.left = `${left}px`
  tip.style.top = `${top}px`
  tip.classList.add('on')
}

export function tip(node: HTMLElement, content: TipContent | null) {
  let c = content
  const enter = () => {
    if (!c) return
    clearTimeout(timer)
    timer = setTimeout(() => c && show(node, c), DELAY)
  }
  node.addEventListener('mouseenter', enter)
  node.addEventListener('focus', enter)
  node.addEventListener('mouseleave', hide)
  node.addEventListener('blur', hide)
  node.addEventListener('mousedown', hide)
  return {
    update(next: TipContent | null) {
      c = next
    },
    destroy() {
      node.removeEventListener('mouseenter', enter)
      node.removeEventListener('focus', enter)
      node.removeEventListener('mouseleave', hide)
      node.removeEventListener('blur', hide)
      node.removeEventListener('mousedown', hide)
      hide()
    },
  }
}

if (typeof window !== 'undefined') {
  addEventListener('scroll', hide, true)
  addEventListener('keydown', hide, true)
}
