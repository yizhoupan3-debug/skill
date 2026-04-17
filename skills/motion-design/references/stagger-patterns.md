# Stagger Entrance Patterns

Orchestrating the sequence of entrance is key to the "Premium" feel.

## The "100ms Rule"
Children should generally stay within **50ms-100ms** of each other. Anything slower feels sluggish; anything faster feels like a single block.

## Pattern A: List / Grid Entrance
Recommended for: Product grids, list items, features.

```jsx
const container = {
  hidden: { opacity: 0 },
  show: {
    opacity: 1,
    transition: {
      staggerChildren: 0.1,
      delayChildren: 0.3
    }
  }
}

const item = {
  hidden: { opacity: 0, y: 20 },
  show: { opacity: 1, y: 0 }
}
```

## Pattern B: Text Reveal
Recommended for: Hero titles, headings.

Reveal by **word** or **character** using a tight stagger (0.02s-0.05s) for a modern, high-end editorial feel.

## Directional Logic
- **Bottom-to-Top**: Default (rising into view).
- **Left-to-Right**: Reading direction (content reveal).
- **Center-Out**: Focus / Interaction point.
