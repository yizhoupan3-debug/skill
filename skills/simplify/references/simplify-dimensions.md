# Simplify Dimensions (Lens Catalog)

> 可扩展的简化审查维度目录。
> `/simplify` 的三个子代理（复用/质量/效率）从本目录选择适用维度。
> 新维度可随时追加到对应章节；添加时必须填写所有字段（维度 ID、名称、审查要点、正例、反例、严重程度）。

---

## 复用维度 (Reuse)

### R1: 重复逻辑

**名称**: 消除重复代码

**审查要点**:
1. 完全重复：逐行相同的代码块出现在多处
2. 结构重复：相同控制流 + 不同常量/变量（参数化即可统一）
3. 算法重复：不同实现但语义等价的逻辑（如两种方式做同一过滤）
4. 跨文件重复：不同模块中的平行实现
5. 测试与生产代码的重复逻辑（可提取为共享 helper）

**正例**:
```python
# Before: 三处相同的价格计算
total_a = price * qty * (1 - discount)
total_b = price * qty * (1 - discount)
total_c = price * qty * (1 - discount)

# After: 提取为函数
def calc_total(price, qty, discount):
    return price * qty * (1 - discount)
```

**反例**:
```python
# 不应合并：表面相似但业务语义不同
def calc_order_total(price, qty, discount):
    return price * qty * (1 - discount)

def calc_refund_total(price, qty, restocking_fee):
    # 退款计算与订单计算相似，但政策/舍入/限额完全不同
    return price * qty * (1 - restocking_fee) - restocking_fee
```

**严重程度**: P1（完全重复）、P2（结构重复）、P3（算法重复）

---

### R2: 已有工具函数

**名称**: 复用现有实现

**审查要点**:
1. 项目内是否已有同功能的工具函数/模块（grep + 测试确认）
2. 标准库/built-in 是否提供等价实现（如 `map`/`filter` vs 手写循环）
3. 已引入的依赖是否提供更优实现（如 `lodash`、`itertools`）
4. 团队内部库/共享包是否覆盖此需求
5. 新代码是否忽略了已有的 wrapper/helper 直接调底层 API

**正例**:
```rust
// Before: 手写字符串截断
let truncated = if s.len() > 100 { &s[..100] } else { s };

// After: 使用已有 crate 或标准库
let truncated = s.chars().take(100).collect::<String>();
```

**反例**:
```rust
// 不应替换：已有实现性能更优且语义清晰
// 手写 SIMD 匹配器 vs 通用 regex
let matched = simd_find_pattern(&haystack, &pattern);
// 不要为了 "复用" 而换成 regex——这里是刻意的性能优化
```

**严重程度**: P1（完全可用的标准库替代）、P2（项目内已有实现）、P3（第三方库替代）

---

### R3: 可复用组件

**名称**: 提取通用组件/管道

**审查要点**:
1. 相似 UI 组件（两个以上页面有布局/交互几乎相同的组件）
2. 相似数据处理管道（读取 → 转换 → 验证 → 存储的流程在多处重复）
3. 相似 API handler 模式（参数解析 → 业务逻辑 → 响应格式化）
4. 相似配置解析逻辑（多处用相同方式解析同类配置）
5. 相似错误处理模式（相同的 try/catch 或 Result 匹配在多处出现）

**正例**:
```tsx
// Before: 两个页面各自实现卡片列表
function UserList() { /* 翻页、空状态、加载骨架屏 */ }
function ProductList() { /* 同样逻辑，仅数据源不同 */ }

// After: 提取通用列表组件
function PaginatedList<T>({ fetchFn, renderItem, emptyMsg }) { ... }
```

**反例**:
```tsx
// 不应提取：仅表面相似，交互和状态管理完全不同
// UserList 有拖拽排序、内联编辑、批量操作
// ProductList 有筛选面板、价格计算器
// 强行抽取会导致泛型爆炸或大量 prop callback
```

**严重程度**: P2（建议统一）、P3（可选抽取）

---

### R4: 标准库/API 替代

**名称**: 优先使用标准库

**审查要点**:
1. 自实现排序 → 标准库 `.sort()` / `.sorted()`
2. 手动解析数字/日期 → 标准库解析函数
3. 自写 hash/dedup → 标准库 `Set` / `HashMap` / `.dedup()`
4. 手动字符串拼接 → 模板字符串 / `join` / `format!`
5. 自实现缓存/LRU → 标准库或成熟的 cache crate

**正例**:
```rust
// Before: 手动冒泡排序
fn bubble_sort(v: &mut Vec<i32>) { /* 30 行 */ }

// After
v.sort();
```

**反例**:
```rust
// 不应替换：自定义排序有特定业务语义
fn priority_sort(tasks: &mut Vec<Task>) {
    // 先按紧急程度，再按截止日期，最后按创建时间
    // 标准库 sort() 无法一步表达这个三级排序的业务规则
    tasks.sort_by(|a, b| a.urgency.cmp(&b.urgency)
        .then(a.deadline.cmp(&b.deadline))
        .then(a.created.cmp(&b.created)));
}
// 这里用 sort_by 是正确的——不应替换成 "更简单" 的 sort()
```

**严重程度**: P1（完全等价的标准库替代）、P2（标准库更简洁但需适配）

---

## 质量维度 (Quality)

### Q1: 命名

**名称**: 改善命名清晰度

**审查要点**:
1. 模糊名称：`data`、`item`、`result`、`temp`、`obj` 等不传达语义
2. 误导性名称：函数名暗示一种行为但实际做另一种（如 `validate` 实际做转换）
3. 过于通用的名称：`process`、`handle`、`doStuff` 没有上下文信息
4. 缩写滥用：`usr`、`mgr`、`ctx`、`cfg` 在非约定上下文中不清晰
5. 类型/命名不一致：布尔变量不是 `is_`/`has_` 前缀，集合不用复数

**正例**:
```rust
// Before
fn process(d: &Data) -> Result<Item, Error> { ... }

// After
fn parse_user_preferences(raw: &RawConfig) -> Result<UserPrefs, ConfigError> { ... }
```

**反例**:
```rust
// 不应改：项目内已有明确约定
// ctx 在此项目中是 Context 的标准缩写，所有文件一致使用
let ctx = AppContext::new();
// 不要改成 app_context——团队约定 > 通用最佳实践
```

**严重程度**: P1（误导性名称）、P2（模糊名称）、P3（风格偏好）

---

### Q2: 函数分解

**名称**: 合理拆分函数

**审查要点**:
1. 超长函数：单函数超过 50 行（或项目约定阈值）
2. 混合关注点：同一函数处理 I/O + 业务逻辑 + 格式化
3. 单一职责违反：函数同时做解析 + 验证 + 持久化
4. 参数过多：超过 4-5 个参数，暗示应使用配置对象或拆分
5. 抽象层次混合：高层编排逻辑中混入底层实现细节

**正例**:
```python
# Before: 80 行函数混合 IO + 业务 + 格式化
def import_users(csv_path):
    # 读文件 (20行)
    # 解析CSV (20行)
    # 验证数据 (20行)
    # 写数据库 (20行)

# After: 三层分离
def import_users(csv_path):
    records = parse_csv(csv_path)
    validated = validate_users(records)
    return persist_users(validated)
```

**反例**:
```python
# 不应拆分：已经足够内聚，拆分反而增加间接层
def calculate_tax(income, brackets):
    tax = 0
    remaining = income
    for bracket in brackets:
        taxable = min(remaining, bracket.limit - bracket.start)
        tax += taxable * bracket.rate
        remaining -= taxable
        if remaining <= 0:
            break
    return tax
# 15 行，单一职责，拆分无收益
```

**严重程度**: P1（超过 100 行的超长函数）、P2（50-100 行或混合关注点）、P3（略超阈值但内聚）

---

### Q3: 控制流

**名称**: 简化控制流

**审查要点**:
1. 深层嵌套（超过 3 层）→ 提取守卫子句 + early return
2. 复杂条件表达式 → 提取为命名谓词函数
3. 长 if-else 链 → 查找表 / 策略模式 / 匹配表达式
4. 嵌套三元运算符 → 提取变量或拆分函数
5. 标记变量（`found`、`done`）→ 直接返回或使用迭代器方法

**正例**:
```go
// Before: 4 层嵌套
func processOrder(o Order) error {
    if o.IsValid() {
        if o.HasStock() {
            if o.PaymentOK() {
                // ... 核心逻辑
            }
        }
    }
    return nil
}

// After: 守卫子句
func processOrder(o Order) error {
    if !o.IsValid()   { return ErrInvalid }
    if !o.HasStock()  { return ErrOutOfStock }
    if !o.PaymentOK() { return ErrPayment }
    // ... 核心逻辑（无嵌套）
    return nil
}
```

**反例**:
```go
// 不应简化：嵌套反映业务层次，早返回会丢失上下文
func (s *Server) handleRequest(w http.ResponseWriter, r *http.Request) {
    if r.Method == "POST" {
        if s.auth(r) {
            if s.rateLimit(r) {
                s.handleWrite(w, r)
            } else {
                // 需要在这里记录限流日志，包含 auth 信息
                s.logRateLimit(r)
            }
        } else {
            // 需要记录未授权尝试
            s.logAuthFailure(r)
        }
    }
    // 每个分支都有不同副作用，守卫子句会丢失这些分支逻辑
}
```

**严重程度**: P2（深层嵌套）、P2（复杂条件）、P3（风格偏好）

---

### Q4: 代码气味

**名称**: 消除代码气味

**审查要点**:
1. 死代码：未使用的函数、变量、import、配置项
2. 注释代码：被注释掉的旧实现（应由 VCS 保留历史）
3. 魔法数字/字符串：未命名的常量（如 `if status == 3`）
4. Stringly-typed：用字符串枚举应使用 enum/union type 的场景
5. 过度 nullable：可选字段过多、null 检查层层传递（应用 Option/Maybe）

**正例**:
```typescript
// Before
if (user.role === "admin" || user.role === "superadmin") { ... }

// After
type Role = "admin" | "superadmin" | "viewer";
const isAdmin = (role: Role) => role === "admin" || role === "superadmin";
if (isAdmin(user.role)) { ... }
```

**反例**:
```typescript
// 不应视为代码气味：有意的灵活性
// 配置文件解析器必须接受任意字符串键
// 用 Record<string, unknown> 是正确的——不是 "stringly-typed"
function parseConfig(raw: Record<string, unknown>): Config {
    // ... 宽松解析，因为配置来源是外部 YAML
}
```

**严重程度**: P1（死代码 + 注释代码）、P2（魔法数字 + stringly-typed）、P3（风格偏好）

---

### Q5: 过度工程

**名称**: 去除不必要的抽象

**审查要点**:
1. 单实现接口：只有 1 个实现的 interface/trait（YAGNI）
2. 工厂/策略/构建器 for 单消费者：只为一个调用方创建的抽象层
3. Premature abstraction：基于两个相似点就抽取泛型，忽略差异点
4. 过度配置化：硬编码完全够用的常量被提取为配置项
5. 抽象层叠加：A → B → C → 实际逻辑，每层只做转发

**正例**:
```java
// Before: 工厂只为一个消费者服务
interface NotificationSender { void send(Notification n); }
class EmailNotificationSender implements NotificationSender { ... }
class NotificationSenderFactory {
    static NotificationSender create(Config c) { return new EmailNotificationSender(); }
}

// After: 直接使用
class EmailService {
    void sendEmail(Notification n) { ... }
}
```

**反例**:
```java
// 不应去抽象：虽然目前只有 1 个实现，但接口承载契约语义
interface UserRepository {
    Optional<User> findById(UserId id);
    List<User> findByEmail(Email email);
}
class PostgresUserRepository implements UserRepository { ... }
// Repository 模式是领域契约，不是过度工程——DDD 中这是标准做法
```

**严重程度**: P2（明显的单实现接口）、P3（可选的配置化/抽象层）

---

## 效率维度 (Efficiency)

### E1: 冗余计算

**名称**: 消除重复计算

**审查要点**:
1. 循环内不变量：循环体内计算不依赖迭代变量的值
2. 重复调用：同一函数在短时间内被多次调用且结果不变（可用缓存/memo）
3. 过度序列化：同一数据被序列化/反序列化多次
4. 重复构建：每次请求都构建相同的正则/模式对象
5. 未利用前一次计算结果：如排序后又过滤再重新排序

**正例**:
```rust
// Before: 循环内重复计算
for item in &items {
    let threshold = config.max_value * 0.8; // 每次迭代都重新计算
    if item.score > threshold { ... }
}

// After: 提到循环外
let threshold = config.max_value * 0.8;
for item in &items {
    if item.score > threshold { ... }
}
```

**反例**:
```rust
// 不应优化：值在循环中可能被修改
let mut threshold = config.base;
for item in &items {
    threshold += item.bonus; // threshold 依赖迭代
    if item.score > threshold { ... }
}
```

**严重程度**: P1（性能关键路径中的明显冗余）、P2（一般路径中的冗余计算）、P3（微优化）

---

### E2: N+1 查询模式

**名称**: 消除 N+1 数据访问

**审查要点**:
1. 循环内数据库查询：遍历列表时对每个元素执行 DB 查询
2. 循环内网络调用：遍历时逐个发起 HTTP 请求（可批量）
3. 循环内文件 I/O：逐行读写而非批量操作
4. 懒加载触发风暴：首次访问时级联加载大量关联数据
5. 未预加载关联数据：ORM 中缺少 `include`/`eager loading`

**正例**:
```sql
-- Before: N+1 (循环内查询)
-- for each user_id:
--   SELECT * FROM orders WHERE user_id = ?

-- After: 批量查询
SELECT * FROM orders WHERE user_id IN (?, ?, ?, ...)
```

**反例**:
```sql
-- 不应批量：数据量极大，IN 子句会导致查询计划退化
-- 应使用游标/分页而非 IN（百万级 ID 列表）
SELECT * FROM orders WHERE user_id IN (/* 100万个ID */)
-- 正确做法：分页查询或使用临时表
```

**严重程度**: P1（循环内 DB/网络调用）、P2（可优化的懒加载）

---

### E3: 不必要的分配

**名称**: 减少内存分配

**审查要点**:
1. 频繁创建临时对象：循环内创建的短生命周期对象可用重用/池化
2. `String` vs `&str`（Rust）：不必要的堆分配，借用即可
3. `Vec` 未预分配：已知大小但未调用 `with_capacity`
4. 不必要的 `clone()`：仅为满足借用检查而克隆（应调整生命周期）
5. 频繁拼接字符串：应使用 `StringBuilder` / `BufWriter` / `format!`

**正例**:
```rust
// Before
let mut results = Vec::new();
for i in 0..1000 {
    results.push(format!("item_{}", i)); // 每次 push 可能 realloc
}

// After
let mut results = Vec::with_capacity(1000);
for i in 0..1000 {
    results.push(format!("item_{}", i));
}
```

**反例**:
```rust
// 不应优化：clone 是为了语义正确性
fn update_config(&mut self, new_cfg: Config) {
    self.history.push(self.current_config.clone()); // 历史快照必须独立副本
    self.current_config = new_cfg;
}
```

**严重程度**: P1（热路径中的频繁分配）、P2（一般路径）、P3（微优化）

---

### E4: 并发机会

**名称**: 利用并发/并行

**审查要点**:
1. 独立 I/O 操作可并行：多个 API 调用之间无依赖
2. CPU 密集任务可分线程：图像处理、大量计算可用线程池
3. 异步未充分利用：同步等待本可异步执行的操作
4. 批处理可分片：大数组可并行处理（如 `rayon`、`Promise.all`）
5. 流水线化：读取-处理-写入可使用 producer-consumer 模式

**正例**:
```typescript
// Before: 串行调用
const users = await fetchUsers();
const orders = await fetchOrders();
const products = await fetchProducts();

// After: 并行调用
const [users, orders, products] = await Promise.all([
    fetchUsers(), fetchOrders(), fetchProducts()
]);
```

**反例**:
```typescript
// 不应并行：存在数据依赖
const user = await fetchUser(id);
const orders = await fetchOrders(user.region); // 依赖 user 的数据
```

**严重程度**: P2（明显的并发机会）、P3（收益不确定的并行化）

---

### E5: 前端渲染优化

**名称**: 减少不必要的渲染开销

**审查要点**:
1. 不必要的 re-render：父组件更新导致未变化的子组件重新渲染
2. 大列表未虚拟化：超过 100 项的列表未使用虚拟滚动
3. 图片未懒加载：首屏外的图片应使用 `loading="lazy"` 或 IntersectionObserver
4. 过大的 bundle：可代码分割的模块被整体加载
5. 频繁重计算：昂贵的派生值未使用 `useMemo`/`computed` 缓存

**正例**:
```tsx
// Before
function List({ items }) {
    return items.map(item => <ExpensiveItem key={item.id} item={item} />);
}

// After: React.memo 防止未变化的 item 重渲染
const ExpensiveItem = React.memo(function ExpensiveItem({ item }) {
    return <div>{/* ... */}</div>;
});
```

**反例**:
```tsx
// 不应 memo：组件极轻量，memo 的比较开销 > 重渲染开销
const Badge = React.memo(({ count }) => <span>{count}</span>);
// 一个 span 的渲染成本几乎为零，memo 反而增加内存和比较成本
```

**严重程度**: P2（明显的渲染浪费）、P3（微优化）

---

## Rust 特化 (Rust-specific)

### RS1: Clippy Lints

**名称**: 遵循 Clippy 建议

**审查要点**:
1. `clippy::needless_return`：函数末尾多余的 `return`
2. `clippy::redundant_clone`：可避免的 `.clone()`
3. `clippy::manual_map`：`match` + 构造 → `.map()`
4. `clippy::needless_borrow`：多余的 `&` 引用
5. `clippy::single_match`：单分支 `match` → `if let`

**正例**:
```rust
// Before
let result = match opt_val {
    Some(v) => Some(transform(v)),
    None => None,
};

// After
let result = opt_val.map(transform);
```

**反例**:
```rust
// 不应改写：match 保留是为了可读性和未来扩展
match state {
    Running => handle_running(),
    Stopped => handle_stopped(),
    // 团队约定：状态机始终用 match 而非 if-let
    // 方便后续添加新状态
}
```

**严重程度**: P2（大部分 Clippy lint）、P1（`redundant_clone` 在热路径中）

---

### RS2: 生命周期简化

**名称**: 简化生命周期标注

**审查要点**:
1. 不必要的 `'static`：数据不需要整个程序生命周期，但被标注为 `'static`
2. 可省略的生命周期标注：符合省略规则（lifetime elision rules）的显式标注
3. `&String` → `&str`：函数参数接受 `&String` 而非 `&str`
4. `&Vec<T>` → `&[T]`：函数参数接受 `&Vec<T>` 而非 `&[T]`
5. `&Box<T>` → `&T`：不必要的间接引用

**正例**:
```rust
// Before
fn greet(name: &String) -> String {
    format!("Hello, {}", name)
}

// After
fn greet(name: &str) -> String {
    format!("Hello, {}", name)
}
```

**反例**:
```rust
// trait object 场景下 &'static dyn Trait 是有意为之
trait DataStore: Send + Sync + 'static {
    fn get(&self, key: &str) -> Option<Vec<u8>>;
}
// 'static 约束确保 trait object 可安全跨线程传递
fn spawn_worker(store: &'static dyn DataStore) {
    std::thread::spawn(move || { store.get("key"); });
}
```

**严重程度**: P2（可简化的签名）、P1（`&String` → `&str` 在公开 API 中）

---

### RS3: 所有权优化

**名称**: 优化所有权与借用

**审查要点**:
1. 可避免的 `clone()`：可通过调整参数顺序或使用借用避免
2. `Cow<'_, T>` 适用场景：读多写少的数据，有时借用有时拥有
3. 借用 vs 转移：函数不需要所有权时不应要求 `T`，应使用 `&T`
4. `to_string()` vs `into()`：可用更高效的类型转换
5. 不必要的 `Arc`/`Rc`：单一所有者即可满足的场景被包装为共享所有权

**正例**:
```rust
// Before
fn process(data: Vec<u8>) {
    let first = data[0];
    // 只读取第一个字节，却拿走了整个 Vec 的所有权
    println!("{}", first);
}

// After
fn process(data: &[u8]) {
    let first = data[0];
    println!("{}", first);
}
```

**反例**:
```rust
// 不应优化：所有权转移是有意的——消费后不再需要
fn consume_task(task: Task) -> TaskResult {
    // task 被消费，内部状态被转移，不应改为 &Task
    TaskResult { id: task.id, status: task.execute() }
}
```

**严重程度**: P2（可避免的 clone）、P3（所有权风格偏好）

---

### RS4: Trait 设计

**名称**: 改善 Trait 设计

**审查要点**:
1. 过于宽泛的 trait bound：`T: Clone + Debug + Serialize + Deserialize + 'static` 中有不必要的约束
2. 不必要的 `dyn dispatch`：只有 1-2 个实现时不需要 trait object，可用泛型或 enum
3. blanket impl 滥用：`impl<T> MyTrait for T where T: ...` 导致实现冲突或语义不清
4. 过大的 trait：一个 trait 有 10+ 方法，应拆分为多个小 trait
5. 不必要的 trait 抽象：仅用于测试 mock 但生产中只有单一实现

**正例**:
```rust
// Before: 不必要的 dyn dispatch
fn process(handler: &dyn Handler) { handler.handle(); }

// After: 只有一个实现时，直接使用具体类型
fn process(handler: &ConcreteHandler) { handler.handle(); }
```

**反例**:
```rust
// 不应移除 dyn：合理的 trait object 用于插件系统
trait Plugin {
    fn name(&self) -> &str;
    fn execute(&self, ctx: &mut Context);
}
// 多个运行时加载的插件实现，dyn 是正确选择
fn run_plugins(plugins: &[Box<dyn Plugin>]) { ... }
```

**严重程度**: P2（不必要的 dyn dispatch）、P3（trait 设计偏好）

---

## 扩展指南

### 添加新维度

1. **选择章节**：确定新维度属于复用(Reuse)、质量(Quality)、效率(Efficiency) 或语言特化
2. **分配 ID**：使用章节前缀 + 递增数字（如 `R5`、`Q6`、`E6`、`RS5`）
3. **填写所有字段**：维度 ID、名称、审查要点(3-5条)、正例、反例、严重程度
4. **正例必须可操作**：包含 before/after 代码对比，而非抽象描述
5. **反例必须有理由**：说明为什么在该场景下不应简化，避免误用
6. **更新本文件**：追加到对应章节末尾，保持 ID 连续

### 为新语言添加特化维度

参照 Rust 特化章节的结构，为新语言创建对应章节：

- **TypeScript 特化 (TS-specific)**：如 `TS1: 类型体操简化`、`TS2: any/unknown 替换`、`TS3: async/await 模式`
- **Python 特化 (PY-specific)**：如 `PY1: 列表推导 vs 循环`、`PY2: 装饰器滥用`、`PY3: 类型注解补全`
- **Go 特化 (GO-specific)**：如 `GO1: error wrapping`、`GO2: 接口粒度`、`GO3: goroutine 泄漏`

新语言章节的格式要求：
1. 章节标题格式：`## {语言} 特化 ({Language}-specific)`
2. 维度 ID 格式：`{LANG}1`、`{LANG}2` 等（2-4 位大写字母前缀）
3. 每个维度遵循相同的 6 字段结构
4. 正例/反例必须使用对应语言的代码

### 维度生命周期

- **Draft**：新维度默认为 Draft，标记为 `[Draft]` 后缀
- **Active**：经过 3 次以上实际使用验证后升级为 Active
- **Deprecated**：不再适用的维度标记为 `[Deprecated]`，保留在文件中但不被选择
