# Spring Physics Presets

Use these presets for a consistent, premium feel across the interface. Grounded in Framer Motion / Motion.dev values.

| Preset | Stiffness | Damping | Character | Usage |
|--------|-----------|---------|-----------|-------|
| **Premium (Default)** | 300 | 20 | Snappy, professional, subtle bounce | Buttons, menu items, small translates |
| **Soft (Playful)** | 200 | 25 | Slower, gentle, friendly | Modals opening, large cards, background shifts |
| **Rigid (Alert)** | 500 | 30 | Immediate, no bounce, secure | Form validation errors, critical alerts |
| **Bouncy (Dynamic)**| 400 | 15 | High energy, noticeable overshoot | Success states, festive elements, gamified UI |

## Framer Motion Snippet

```jsx
<motion.div
  transition={{
    type: "spring",
    stiffness: 300,
    damping: 20
  }}
/>
```

## CSS Cubic-Bezier Equivalent (Approx)

While not a true spring, these bezier curves mimic the "acceleration-deceleration" of premium motion:

- **Snappy Out**: `cubic-bezier(0.16, 1, 0.3, 1)`
- **Dynamic Bounce**: `cubic-bezier(0.34, 1.56, 0.64, 1)`
