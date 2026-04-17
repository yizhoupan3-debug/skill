# Svelte 5 Runes Migration Guide

## Reactivity: Old → New

| Svelte 4 | Svelte 5 (Runes) | Notes |
|----------|-------------------|-------|
| `let count = 0` (reactive in component) | `let count = $state(0)` | Explicit reactivity declaration |
| `$: doubled = count * 2` | `let doubled = $derived(count * 2)` | Derived values |
| `$: { console.log(count) }` | `$effect(() => { console.log(count) })` | Side effects |
| `$: if (count > 5) { ... }` | `$effect(() => { if (count > 5) { ... } })` | Conditional effects |
| writable store | `$state()` | Stores still work but runes preferred |
| derived store | `$derived()` | Simpler API |

## Core Runes

### $state
```svelte
<script>
  // Simple state
  let count = $state(0);

  // Object state (deeply reactive)
  let user = $state({ name: 'Joe', age: 25 });

  // Array state
  let items = $state([1, 2, 3]);
</script>

<button onclick={() => count++}>{count}</button>
<p>{user.name}</p>
```

### $derived
```svelte
<script>
  let count = $state(0);
  let doubled = $derived(count * 2);

  // Complex derivation:
  let filtered = $derived.by(() => {
    return items.filter(i => i.active);
  });
</script>
```

### $effect
```svelte
<script>
  let count = $state(0);

  // Auto-tracks dependencies:
  $effect(() => {
    console.log(`count is ${count}`);
    // Cleanup:
    return () => { /* cleanup */ };
  });

  // Pre-effect (runs before DOM update):
  $effect.pre(() => { /* ... */ });
</script>
```

### $props
```svelte
<script>
  // Svelte 4: export let name;
  // Svelte 5:
  let { name, age = 25, ...rest } = $props();
</script>
```

### $bindable
```svelte
<script>
  // Two-way bindable prop:
  let { value = $bindable() } = $props();
</script>

<!-- Parent: -->
<Child bind:value={parentValue} />
```

### $inspect (dev only)
```svelte
<script>
  let count = $state(0);
  // Logs when count changes (dev mode only):
  $inspect(count);
  // With custom handler:
  $inspect(count).with(console.trace);
</script>
```

## Component Events: Old → New

| Svelte 4 | Svelte 5 |
|----------|----------|
| `createEventDispatcher()` | Callback props |
| `on:click={handler}` | `onclick={handler}` |
| `on:click\|preventDefault` | `onclick={(e) => { e.preventDefault(); handler(e) }}` |

```svelte
<!-- Svelte 5: callback props instead of events -->
<script>
  let { onsubmit } = $props();
</script>
<button onclick={() => onsubmit?.('data')}>Submit</button>
```

## Snippets (replaces slots)

```svelte
<!-- Svelte 5: snippets replace slots -->
{#snippet header()}
  <h1>Title</h1>
{/snippet}

{#snippet row(item)}
  <tr><td>{item.name}</td></tr>
{/snippet}

<!-- Render a snippet: -->
{@render header()}
{@render row(item)}
```

## Migration Checklist

- [ ] Replace `$:` reactive statements with `$derived()` or `$effect()`
- [ ] Replace `export let` with `$props()`
- [ ] Replace `createEventDispatcher` with callback props
- [ ] Replace `on:event` with `onevent` (lowercase)
- [ ] Replace slots with `{#snippet}` and `{@render}`
- [ ] Replace writable stores with `$state()` where appropriate
- [ ] Add `$state()` to component-level variables that need reactivity
- [ ] Update event modifiers to inline handlers
- [ ] Test SSR hydration after migration
