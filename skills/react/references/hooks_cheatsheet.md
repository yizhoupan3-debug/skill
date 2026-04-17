# React Hooks Cheatsheet

## Core Hooks

### useState
```jsx
const [state, setState] = useState(initialValue);
// Functional update (when next state depends on prev):
setState(prev => prev + 1);
// Lazy initializer (expensive computation):
const [state, setState] = useState(() => computeExpensiveValue());
```

### useEffect
```jsx
// Run on every render:
useEffect(() => { /* effect */ });
// Run on mount only:
useEffect(() => { /* effect */ }, []);
// Run when deps change:
useEffect(() => { /* effect */ return () => { /* cleanup */ }; }, [dep]);
```

### useRef
```jsx
const ref = useRef(null);         // DOM ref
const valueRef = useRef(0);        // mutable value (no re-render)
```

### useMemo / useCallback
```jsx
// Memoize computation:
const value = useMemo(() => expensiveCalc(a, b), [a, b]);
// Memoize callback:
const handler = useCallback((e) => { /* ... */ }, [dep]);
```
> Only use when there's a measured performance problem or referential equality matters.

### useReducer
```jsx
const [state, dispatch] = useReducer(reducer, initialState);
// With lazy init:
const [state, dispatch] = useReducer(reducer, initialArg, init);
```

### useContext
```jsx
const value = useContext(MyContext);
// Avoid re-renders: split context or use selectors (Zustand/Jotai).
```

---

## React 19+ Hooks

### useTransition
```jsx
const [isPending, startTransition] = useTransition();
startTransition(() => { setState(newValue); });
```

### useDeferredValue
```jsx
const deferredQuery = useDeferredValue(query);
```

### useActionState (React 19)
```jsx
const [state, formAction, isPending] = useActionState(serverAction, initialState);
```

### use (React 19)
```jsx
// Read a promise:
const data = use(dataPromise);
// Read context (replaces useContext):
const theme = use(ThemeContext);
```

---

## Common Patterns

### Custom Hook Template
```tsx
function useCustomHook(param: string) {
  const [data, setData] = useState<Data | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    fetchData(param)
      .then(d => { if (!cancelled) setData(d); })
      .catch(e => { if (!cancelled) setError(e); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [param]);

  return { data, error, loading };
}
```

### Dependency Array Rules
| Scenario | Dependencies |
|----------|-------------|
| Run once on mount | `[]` |
| Run on specific change | `[dep1, dep2]` |
| Run every render | omit array |
| Ref values | refs DON'T need to be in deps |
| Stable functions (dispatch, setState) | DON'T need to be in deps |

### Anti-patterns to Avoid
- ❌ Object/array literals in deps (create new ref each render)
- ❌ Missing deps (stale closures)
- ❌ `eslint-disable-next-line react-hooks/exhaustive-deps`
- ❌ setState in useEffect without cleanup (race conditions)
- ❌ useMemo/useCallback everywhere (premature optimization)
