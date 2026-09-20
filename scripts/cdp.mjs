// Drive the running app's webview over the DevTools protocol (needs the app started with
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222; `just dev-debug` does that).
// Usage: node scripts/cdp.mjs <cmd> [args]
//   eval <js>                    run JS in the page, print JSON result
//   click <css-selector|x,y> [n] click element center (or viewport x,y); n=2 double-click
//   shot [name]                  -> debug/screenshots/<name>.png (viewport, no OS involvement)
//   drag <css|x,y> <dx> <dy>     press, move, release (pointer drags)
//   key <Key>                    press a key on the focused element (e.g. Enter, Delete, ArrowDown)
import fs from 'node:fs'
import path from 'node:path'

const PORT = process.env.CDP_PORT ?? 9222
const [cmd, ...args] = process.argv.slice(2)

const targets = await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()
const page = targets.find((t) => t.type === 'page')
if (!page) throw new Error('no page target; is the app running with the debug port?')

const ws = new WebSocket(page.webSocketDebuggerUrl)
await new Promise((r) => (ws.onopen = r))
let id = 0
const pending = new Map()
ws.onmessage = (m) => {
  const d = JSON.parse(m.data)
  if (d.id && pending.has(d.id)) {
    pending.get(d.id)(d)
    pending.delete(d.id)
  }
}
const send = (method, params = {}) =>
  new Promise((res, rej) => {
    const i = ++id
    pending.set(i, (d) => (d.error ? rej(new Error(JSON.stringify(d.error))) : res(d.result)))
    ws.send(JSON.stringify({ id: i, method, params }))
  })
const evaluate = async (expression) => {
  const r = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true })
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? 'eval error')
  return r.result.value
}

async function center(target) {
  if (/^\d+,\d+$/.test(target)) return target.split(',').map(Number)
  const r = await evaluate(`(() => { const e = document.querySelector(${JSON.stringify(target)}); if (!e) return null;
    e.scrollIntoView({block:'nearest'}); const b = e.getBoundingClientRect(); return [b.x + b.width/2, b.y + b.height/2] })()`)
  if (!r) throw new Error(`no element for ${target}`)
  return r
}

if (cmd === 'eval') {
  console.log(JSON.stringify(await evaluate(args.join(' ')), null, 1))
} else if (cmd === 'click') {
  const [x, y] = await center(args[0])
  const count = Number(args[1] ?? 1)
  for (let c = 1; c <= count; c++) {
    const base = { x, y, button: 'left', clickCount: c }
    await send('Input.dispatchMouseEvent', { type: 'mousePressed', ...base })
    await send('Input.dispatchMouseEvent', { type: 'mouseReleased', ...base })
  }
} else if (cmd === 'drag') {
  // drag <css|x,y> <dx> <dy>: press on the target, move in steps, release
  const [x, y] = await center(args[0])
  const [dx, dy] = [Number(args[1]), Number(args[2])]
  await send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 })
  for (let i = 1; i <= 10; i++)
    await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: x + (dx * i) / 10, y: y + (dy * i) / 10, button: 'left', buttons: 1 })
  await send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: x + dx, y: y + dy, button: 'left', clickCount: 1 })
} else if (cmd === 'key') {
  await send('Input.dispatchKeyEvent', { type: 'keyDown', key: args[0], code: args[0] })
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: args[0], code: args[0] })
} else if (cmd === 'shot') {
  const name = args[0] ?? new Date().toISOString().replace(/[:.]/g, '-')
  const { data } = await send('Page.captureScreenshot', { format: 'png' })
  const dir = path.resolve(import.meta.dirname, '..', 'debug', 'screenshots')
  fs.mkdirSync(dir, { recursive: true })
  const out = path.join(dir, `${name}.png`)
  fs.writeFileSync(out, Buffer.from(data, 'base64'))
  console.log(out)
} else {
  console.error('unknown command')
  process.exit(1)
}
ws.close()
