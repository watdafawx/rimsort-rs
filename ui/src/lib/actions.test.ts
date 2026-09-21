// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { dialogFocus } from './actions'

afterEach(() => {
  document.body.innerHTML = ''
})

function dialog() {
  document.body.innerHTML = `<button id="outside">out</button>
    <div id="d" tabindex="-1"><button id="a">a</button><input id="b" /><button id="c">c</button></div>`
  const d = document.getElementById('d')!
  const action = dialogFocus(d)
  // jsdom has no layout, so offsetParent is null; pretend everything is visible.
  for (const el of d.querySelectorAll('*')) Object.defineProperty(el, 'offsetParent', { value: d })
  return { d, action }
}

const tab = (el: Element, shiftKey = false) => {
  const e = new KeyboardEvent('keydown', { key: 'Tab', shiftKey, bubbles: true, cancelable: true })
  el.dispatchEvent(e)
  return e.defaultPrevented
}

describe('dialogFocus', () => {
  it('focuses the dialog on open', async () => {
    const { d } = dialog()
    await Promise.resolve()
    expect(document.activeElement).toBe(d)
  })

  it('leaves focus alone when a field inside already has it', async () => {
    document.body.innerHTML = '<div id="d" tabindex="-1"><input id="i" /></div>'
    const d = document.getElementById('d')!
    const i = document.getElementById('i')!
    i.focus()
    dialogFocus(d)
    await Promise.resolve()
    expect(document.activeElement).toBe(i)
  })

  it('wraps Tab at the end and Shift+Tab at the start', () => {
    const { d } = dialog()
    const [a, , c] = ['a', 'b', 'c'].map((id) => document.getElementById(id)!)
    c.focus()
    expect(tab(c)).toBe(true)
    expect(document.activeElement).toBe(a)
    expect(tab(a, true)).toBe(true)
    expect(document.activeElement).toBe(c)
    // a Tab in the middle is left to the browser
    document.getElementById('b')!.focus()
    expect(tab(document.getElementById('b')!)).toBe(false)
    expect(d).toBeTruthy()
  })

  it('stops listening when destroyed', () => {
    const { action } = dialog()
    const c = document.getElementById('c')!
    action.destroy()
    c.focus()
    expect(tab(c)).toBe(false)
  })
})
