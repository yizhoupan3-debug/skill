# Frontend Debugging Checklists

## Common Pattern Checklists

### Blank screen / white screen
1. Check console for JavaScript errors (uncaught exceptions crash the app)
2. Check Network tab for failed chunk loads (lazy imports)
3. Check if error boundary exists and is rendering fallback
4. Check SSR hydration for mismatch that causes React to bail
5. Check if the entry point HTML has the correct root element
6. Check if environment variables are missing in the build

### Component not updating
1. Verify state is actually changing (React DevTools / Vue DevTools)
2. Check for stale closure in useEffect/useCallback dependency arrays
3. Check for accidental mutation instead of immutable update
4. Check if React.memo or shouldComponentUpdate is blocking
5. Check if the component is unmounted and remounted (wrong key)

### Event handler not firing
1. Check if the element is covered by another element (z-index / overlay)
2. Check if `pointer-events: none` is set
3. Check if `preventDefault()` or `stopPropagation()` is called upstream
4. Check if the handler is bound correctly (arrow function vs method)
5. Check if the event listener type matches (click vs touchend on mobile)

### Hydration mismatch
1. Check for Date.now(), Math.random(), or other non-deterministic values in server render
2. Check for `window` / `document` usage during SSR
3. Check for conditional rendering based on client-only state
4. Check for browser extensions injecting DOM nodes
5. Check for different locale/timezone between server and client

### Third-party library conflict
1. Check for CSS global styles that override your styles
2. Check for multiple versions of the same library in the bundle
3. Check for global variable pollution (window.xxx)
4. Check for event listener leaks from library initialization
5. Check for peer dependency version mismatches

## DevTools Workflow Guide

### Console panel
- Filter by error level to find critical issues first
- Check for React/Vue specific warnings (they often indicate the root cause)
- Use `console.trace()` to find call origin
- Watch for uncaught Promise rejections

### Elements panel
- Verify DOM structure matches expected component tree
- Check computed styles for unexpected overrides
- Use "Break on subtree modifications" to catch unexpected DOM changes
- Check for hidden elements (visibility, display, opacity)

### Network panel
- Filter by failed requests (status 4xx/5xx)
- Check for blocked requests (CORS, mixed content)
- Verify chunk/asset loading order
- Check response payloads for unexpected data shapes

### Sources panel
- Set breakpoints in event handlers to verify execution
- Use conditional breakpoints to catch specific state
- Use the call stack to trace execution flow
- Check for source map availability

### Application panel
- Check localStorage/sessionStorage for corrupt state
- Check Service Worker status and cache contents
- Check cookies for auth/session issues
