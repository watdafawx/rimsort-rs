import { invoke } from '@tauri-apps/api/core'

type Level = 'debug' | 'info' | 'warn' | 'error'
const send = (level: Level, args: unknown[]) =>
  invoke('log_frontend', {
    level,
    message: args
      .map((a) =>
        a instanceof Error ? (a.stack ?? a.message) : typeof a === 'string' ? a : JSON.stringify(a),
      )
      .join(' '),
  }).catch(() => {})

/** Mirror console + uncaught errors into the Rust log. */
export function installFrontendLogging() {
  for (const level of ['debug', 'info', 'warn', 'error'] as const) {
    const orig = console[level].bind(console)
    console[level] = (...args: unknown[]) => {
      orig(...args)
      send(level, args)
    }
  }
  addEventListener('error', (e) =>
    send('error', [`uncaught: ${e.message} @ ${e.filename}:${e.lineno}`]),
  )
  addEventListener('unhandledrejection', (e) => {
    // Backend failures (ErrorDto) were already shown as a toast by call(); log them quietly.
    const backend = typeof e.reason?.kind === 'string' && typeof e.reason?.message === 'string'
    if (backend) e.preventDefault()
    send(backend ? 'warn' : 'error', ['unhandled rejection:', e.reason])
  })
}
