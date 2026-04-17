# Vue 3 Composable Patterns

## Basic Composable Template

```vue
// composables/useCounter.ts
import { ref, computed } from 'vue'

/**
 * A simple counter composable.
 * @param initialValue - Starting value (default: 0)
 */
export function useCounter(initialValue = 0) {
  const count = ref(initialValue)
  const doubled = computed(() => count.value * 2)

  function increment() { count.value++ }
  function decrement() { count.value-- }
  function reset() { count.value = initialValue }

  return { count, doubled, increment, decrement, reset }
}
```

## Async Data Fetching

```ts
// composables/useFetch.ts
import { ref, watchEffect, toValue, type Ref, type MaybeRefOrGetter } from 'vue'

export function useFetch<T>(url: MaybeRefOrGetter<string>) {
  const data = ref<T | null>(null) as Ref<T | null>
  const error = ref<Error | null>(null)
  const loading = ref(true)

  watchEffect(async (onCleanup) => {
    const controller = new AbortController()
    onCleanup(() => controller.abort())

    loading.value = true
    error.value = null

    try {
      const res = await fetch(toValue(url), { signal: controller.signal })
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      data.value = await res.json()
    } catch (e) {
      if ((e as Error).name !== 'AbortError') {
        error.value = e as Error
      }
    } finally {
      loading.value = false
    }
  })

  return { data, error, loading }
}
```

## Event Listener

```ts
// composables/useEventListener.ts
import { onMounted, onUnmounted, toValue, type MaybeRefOrGetter } from 'vue'

export function useEventListener<K extends keyof WindowEventMap>(
  target: MaybeRefOrGetter<EventTarget | null>,
  event: K,
  handler: (e: WindowEventMap[K]) => void
) {
  onMounted(() => {
    const el = toValue(target)
    el?.addEventListener(event, handler as EventListener)
  })
  onUnmounted(() => {
    const el = toValue(target)
    el?.removeEventListener(event, handler as EventListener)
  })
}
```

## LocalStorage with Reactivity

```ts
// composables/useLocalStorage.ts
import { ref, watch, type Ref } from 'vue'

export function useLocalStorage<T>(key: string, defaultValue: T): Ref<T> {
  const stored = localStorage.getItem(key)
  const data = ref<T>(stored ? JSON.parse(stored) : defaultValue) as Ref<T>

  watch(data, (newVal) => {
    localStorage.setItem(key, JSON.stringify(newVal))
  }, { deep: true })

  return data
}
```

## Composable Design Rules

| Rule | Rationale |
|------|-----------|
| Prefix with `use` | Convention for composable identification |
| Accept `MaybeRefOrGetter` params | Works with both reactive and static values |
| Return plain object (not reactive) | Allows destructuring without losing reactivity |
| Use `ref` over `reactive` for returns | Better DX, clearer `.value` access |
| Clean up side effects | Use `onUnmounted` or `watchEffect` cleanup |
| Keep composables focused | One concern per composable |
| Type return values explicitly | Better IDE support and documentation |

## Anti-patterns

- ❌ Calling composables outside `<script setup>` or `setup()`
- ❌ Destructuring `reactive()` return values (loses reactivity)
- ❌ Mixing composables with Options API mixins
- ❌ Storing component-specific state in a global composable
- ❌ Using `watch` without cleanup for async operations
