import { describe, expect, it } from 'vitest'
import { movedItems } from './listdiff'

describe('movedItems', () => {
  it('reports nothing for identical lists', () => {
    expect(movedItems(['a', 'b', 'c'], ['a', 'b', 'c'])).toEqual([])
  })
  it('a mod jumping to the front counts as the only move', () => {
    expect(movedItems(['a', 'b', 'c', 'd', 'e'], ['e', 'a', 'b', 'c', 'd'])).toEqual(['e'])
  })
  it('swapping two neighbours moves one of them', () => {
    expect(movedItems(['a', 'b', 'c'], ['b', 'a', 'c']).length).toBe(1)
  })
  it('ignores items that are not in both lists', () => {
    expect(movedItems(['a', 'x', 'b'], ['b', 'a', 'y'])).toHaveLength(1)
  })
  it('handles empty input', () => {
    expect(movedItems([], ['a'])).toEqual([])
    expect(movedItems(['a'], [])).toEqual([])
  })
})
