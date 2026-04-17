# Coding Standards — Detailed Examples

## TypeScript / JavaScript Examples

### Variable Naming

```typescript
// ✅ Descriptive names
const marketSearchQuery = 'election'
const isUserAuthenticated = true
const totalRevenue = 1000

// ❌ Vague names
const q = 'election'
const flag = true
const x = 1000
```

### Function Naming

```typescript
// ✅ Verb-noun pattern
async function fetchMarketData(marketId: string) {}
function calculateSimilarity(a: number[], b: number[]) {}
function isValidEmail(email: string): boolean {}

// ❌ Vague or noun-only
async function market(id: string) {}
function similarity(a, b) {}
```

### Immutability

```typescript
// ✅ Always use spread
const updatedUser = { ...user, name: 'New Name' }
const updatedArray = [...items, newItem]

// ❌ Never mutate directly
user.name = 'New Name'  // forbidden
items.push(newItem)     // forbidden
```

### Error Handling

```typescript
// ✅ Complete error handling
async function fetchData(url: string) {
  try {
    const response = await fetch(url)
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`)
    }
    return await response.json()
  } catch (error) {
    console.error('Fetch failed:', error)
    throw new Error('Failed to fetch data')
  }
}

// ❌ No error handling
async function fetchData(url) {
  const response = await fetch(url)
  return response.json()
}
```

### Async Best Practices

```typescript
// ✅ Parallel when possible
const [users, markets, stats] = await Promise.all([
  fetchUsers(),
  fetchMarkets(),
  fetchStats()
])

// ❌ Unnecessary sequential
const users = await fetchUsers()
const markets = await fetchMarkets()
const stats = await fetchStats()
```

### Type Safety

```typescript
// ✅ Proper type definitions
interface Market {
  id: string
  name: string
  status: 'active' | 'resolved' | 'closed'
  created_at: Date
}

function getMarket(id: string): Promise<Market> { }

// ❌ Using any
function getMarket(id: any): Promise<any> { }
```

## React Best Practices

### Component Structure

```typescript
// ✅ Typed function component
interface ButtonProps {
  children: React.ReactNode
  onClick: () => void
  disabled?: boolean
  variant?: 'primary' | 'secondary'
}

export function Button({
  children,
  onClick,
  disabled = false,
  variant = 'primary'
}: ButtonProps) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`btn btn-${variant}`}
    >
      {children}
    </button>
  )
}
```

### State Updates

```typescript
// ✅ Functional update based on previous value
setCount(prev => prev + 1)

// ❌ May be stale in async scenarios
setCount(count + 1)
```

### Conditional Rendering

```typescript
// ✅ Clear conditional rendering
{isLoading && <Spinner />}
{error && <ErrorMessage error={error} />}
{data && <DataDisplay data={data} />}

// ❌ Ternary hell
{isLoading ? <Spinner /> : error ? <ErrorMessage error={error} /> : data ? <DataDisplay data={data} /> : null}
```

## Python Examples

### Function Naming and Type Annotations

```python
# ✅ snake_case + type annotations
def fetch_market_data(market_id: str) -> dict:
    ...

def calculate_similarity(vec_a: list[float], vec_b: list[float]) -> float:
    ...

# ❌ No types, vague naming
def market(id):
    ...
```

### Error Handling

```python
# ✅ Specific exception catching with context
def fetch_data(url: str) -> dict:
    try:
        response = requests.get(url, timeout=10)
        response.raise_for_status()
        return response.json()
    except requests.HTTPError as e:
        logger.error("HTTP error fetching %s: %s", url, e)
        raise
    except requests.RequestException as e:
        logger.error("Request failed: %s", e)
        raise

# ❌ Bare except
def fetch_data(url):
    try:
        return requests.get(url).json()
    except:
        pass
```

### List Comprehension vs Loop

```python
# ✅ List comprehension (simple cases)
active_users = [u for u in users if u.is_active]

# ✅ Explicit loop with early continue (complex cases, more readable)
def get_active_admins(users: list[User]) -> list[User]:
    result = []
    for user in users:
        if not user.is_active:
            continue
        if not user.is_admin:
            continue
        result.append(user)
    return result
```

## API Design Examples

### REST Conventions

```
GET    /api/markets          # list
GET    /api/markets/:id      # detail
POST   /api/markets          # create
PUT    /api/markets/:id      # full update
PATCH  /api/markets/:id      # partial update
DELETE /api/markets/:id      # delete

# Filter parameters
GET /api/markets?status=active&limit=10&offset=0
```

### Unified Response Format

```typescript
interface ApiResponse<T> {
  success: boolean
  data?: T
  error?: string
  meta?: { total: number; page: number; limit: number }
}

// Success
return NextResponse.json({ success: true, data: markets, meta: { total: 100 } })

// Failure
return NextResponse.json({ success: false, error: 'Invalid request' }, { status: 400 })
```

### Input Validation (Zod)

```typescript
const CreateMarketSchema = z.object({
  name: z.string().min(1).max(200),
  description: z.string().min(1).max(2000),
  endDate: z.string().datetime(),
})

export async function POST(request: Request) {
  try {
    const validated = CreateMarketSchema.parse(await request.json())
    // use validated data
  } catch (error) {
    if (error instanceof z.ZodError) {
      return NextResponse.json({ success: false, error: 'Validation failed', details: error.errors }, { status: 400 })
    }
  }
}
```

## Comment Standards

```typescript
// ✅ Explain "why", not "what"
// Use exponential backoff to avoid overwhelming the API during outage
const delay = Math.min(1000 * Math.pow(2, retryCount), 30000)

// Intentional mutation here for performance on large arrays
items.push(newItem)

// ❌ Stating the obvious
// Increment counter by 1
count++
```

### JSDoc (Required for Public APIs)

```typescript
/**
 * Search markets using semantic similarity.
 *
 * @param query - Natural language search term
 * @param limit - Max results (default 10)
 * @returns Markets sorted by similarity
 * @throws {Error} When OpenAI API fails or Redis unavailable
 */
export async function searchMarkets(query: string, limit = 10): Promise<Market[]> {}
```

## Code Smell Detection

### 1. Function Too Long
```typescript
// ❌ Functions over 40-50 lines
function processMarketData() { /* 100 lines */ }

// ✅ Split into sub-functions
function processMarketData() {
  const validated = validateData()
  const transformed = transformData(validated)
  return saveData(transformed)
}
```

### 2. Deep Nesting
```typescript
// ❌ 5+ levels of nesting
if (user) { if (user.isAdmin) { if (market) { /* ... */ } } }

// ✅ Early return
if (!user) return
if (!user.isAdmin) return
if (!market) return
// main logic
```

### 3. Magic Numbers
```typescript
// ❌
if (retryCount > 3) {}
setTimeout(callback, 500)

// ✅
const MAX_RETRIES = 3
const DEBOUNCE_DELAY_MS = 500
if (retryCount > MAX_RETRIES) {}
setTimeout(callback, DEBOUNCE_DELAY_MS)
```

### 4. Over-commenting
```typescript
// ❌ Explaining self-explanatory code
// Set name to the user's name
name = user.name

// ✅ Only comment on non-obvious logic
```
# Kaizen — Detailed Examples and Patterns

## Continuous Improvement Code Examples

### Iterative Refinement Pattern

```typescript
// Iteration 1: Make it work
const calculateTotal = (items: Item[]) => {
  let total = 0;
  for (let i = 0; i < items.length; i++) {
    total += items[i].price * items[i].quantity;
  }
  return total;
};

// Iteration 2: Make it clear (refactor)
const calculateTotal = (items: Item[]): number =>
  items.reduce((total, item) => total + item.price * item.quantity, 0);

// Iteration 3: Make it robust (add validation)
const calculateTotal = (items: Item[]): number => {
  if (!items?.length) return 0;
  return items.reduce((total, item) => {
    if (item.price < 0 || item.quantity < 0) {
      throw new Error('Price and quantity must be non-negative');
    }
    return total + item.price * item.quantity;
  }, 0);
};
// Each step is complete, tested, and runnable
```

## Poka-Yoke (Error-Proofing) Examples

### Making Invalid States Unrepresentable

```typescript
// ❌ String status can be anything
type OrderBad = { status: string; total: number };

// ✅ Only valid states are possible
type OrderStatus = 'pending' | 'processing' | 'shipped' | 'delivered';
type Order =
  | { status: 'pending'; createdAt: Date }
  | { status: 'processing'; startedAt: Date; estimatedCompletion: Date }
  | { status: 'shipped'; trackingNumber: string; shippedAt: Date }
  | { status: 'delivered'; deliveredAt: Date; signature: string };
// Can't be 'shipped' without a trackingNumber
```

### Branded Types for Boundary Validation

```typescript
// ✅ Validate at boundary, use safely everywhere
type PositiveNumber = number & { readonly __brand: 'PositiveNumber' };

const validatePositive = (n: number): PositiveNumber => {
  if (n <= 0) throw new Error('Must be positive');
  return n as PositiveNumber;
};

const processPayment = (amount: PositiveNumber) => {
  // No need to check again — type guarantees it
  const fee = amount * 0.03;
};

// Validate once at the system boundary
const handlePaymentRequest = (req: Request) => {
  const amount = validatePositive(req.body.amount);
  processPayment(amount);
};
```

### Guard Clauses

```typescript
// ✅ Early return, avoid deep nesting
const processUser = (user: User | null) => {
  if (!user) { logger.error('User not found'); return; }
  if (!user.email) { logger.error('Email missing'); return; }
  if (!user.isActive) { logger.info('User inactive'); return; }

  // Here, user is guaranteed valid and active
  sendEmail(user.email, 'Welcome!');
};
```

### Fail-Fast Configuration

```typescript
// ✅ Fail at startup, not at request time
const loadConfig = (): Config => {
  const apiKey = process.env.API_KEY;
  if (!apiKey) throw new Error('API_KEY environment variable required');
  return { apiKey, timeout: 5000 };
};

// App fails at startup if config is invalid, not during request handling
const config = loadConfig();
```

## Standardized Work Examples

### Consistent API Client Pattern

```typescript
// ✅ New code follows existing patterns
class UserAPIClient {
  async getUser(id: string): Promise<User> {
    return this.fetch(`/users/${id}`);
  }
}

// New code maintains the same pattern
class OrderAPIClient {
  async getOrder(id: string): Promise<Order> {
    return this.fetch(`/orders/${id}`);
  }
}
```

### Unified Error Handling with Result Type

```typescript
// ✅ Consistent error handling across the codebase
type Result<T, E> = { ok: true; value: T } | { ok: false; error: E };

const fetchUser = async (id: string): Promise<Result<User, Error>> => {
  try {
    const user = await db.users.findById(id);
    if (!user) return { ok: false, error: new Error('User not found') };
    return { ok: true, value: user };
  } catch (err) {
    return { ok: false, error: err as Error };
  }
};

const result = await fetchUser('123');
if (!result.ok) { logger.error('Failed', result.error); return; }
const user = result.value; // type-safe!
```

## Just-In-Time Examples

### Evolving Complexity On Demand

```typescript
// ✅ Add complexity only when needed
// V1: Current requirement
const formatCurrency = (amount: number): string => `$${amount.toFixed(2)}`;

// V2: Multi-currency support needed
const formatCurrency = (amount: number, currency: string): string => {
  const symbols = { USD: '$', EUR: '€', GBP: '£' };
  return `${symbols[currency]}${amount.toFixed(2)}`;
};

// V3: Localization needed
const formatCurrency = (amount: number, locale: string): string =>
  new Intl.NumberFormat(locale, { style: 'currency', currency: 'USD' }).format(amount);
// Complexity only added when required
```

### Rule of Three

```typescript
// ✅ Only abstract after a pattern appears 3+ times
// Have two concrete implementations first, extract on third occurrence

// ❌ Building generic frameworks for a single use case
abstract class BaseCRUDService<T> { /* 300 lines */ }
class GenericRepository<T> { /* 200 lines */ }
// Building massive abstractions for uncertain future use
```
