/**
 * Which items of a reordered list actually *moved*, as opposed to merely being pushed along by others.
 * Items on the longest increasing subsequence of new positions keep their relative order; the rest moved.
 * Only items present in both lists are considered.
 */
export function movedItems(before: string[], after: string[]): string[] {
  const pos = new Map(after.map((id, i) => [id, i]))
  const common = before.filter((id) => pos.has(id))
  const seq = common.map((id) => pos.get(id)!)

  // Patience sorting: tails[k] = index into `seq` of the smallest tail of an increasing run of length k+1.
  const tails: number[] = []
  const prev: number[] = new Array(seq.length).fill(-1)
  for (let i = 0; i < seq.length; i++) {
    let lo = 0
    let hi = tails.length
    while (lo < hi) {
      const mid = (lo + hi) >> 1
      if (seq[tails[mid]] < seq[i]) lo = mid + 1
      else hi = mid
    }
    if (lo > 0) prev[i] = tails[lo - 1]
    tails[lo] = i
  }
  const stable = new Set<number>()
  for (let k = tails.length ? tails[tails.length - 1] : -1; k >= 0; k = prev[k]) stable.add(k)
  return common.filter((_, i) => !stable.has(i))
}
