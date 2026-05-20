# Adversarial Review Guide

Full adversarial code review with 6 lens coverage.

## Review Philosophy

Be hostile but fair. Assume the worst. Test the edges.

## 6 Required Lenses

### Lens 1: Correctness

**Focus**: Logic correctness

**Questions**:
- Are there off-by-one errors?
- Are edge cases handled?
- Is error handling correct?
- Are types used correctly?
- Are there race conditions?

**Commands**:
```bash
cargo test
cargo clippy
# Manual code review
```

### Lens 2: Security

**Focus**: Security vulnerabilities

**Questions**:
- Is input validated?
- Are there injection vulnerabilities?
- Is authentication correct?
- Is authorization enforced?
- Is sensitive data protected?
- Are secrets managed properly?

**Commands**:
```bash
cargo audit
# Manual security review
# OWASP checklist
```

### Lens 3: Performance

**Focus**: Performance characteristics

**Questions**:
- Are there O(n²) or worse algorithms?
- Are there memory leaks?
- Are resources properly released?
- Is caching appropriate?
- Are there N+1 queries?
- Is async used correctly?

**Commands**:
```bash
cargo flamegraph  # if available
valgrind  # memory
cargo bench  # performance
```

### Lens 4: Maintainability

**Focus**: Code readability and structure

**Questions**:
- Is the code readable?
- Are modules properly separated?
- Is there appropriate abstraction?
- Are tests comprehensive?
- Is documentation adequate?
- Is naming clear?

**Commands**:
```bash
cargo clippy -- -W clippy::all
cargo fmt -- --check
# Manual readability review
```

### Lens 5: Reliability

**Focus**: Error handling and recovery

**Questions**:
- Does the code fail gracefully?
- Are retries implemented?
- Is idempotency ensured?
- Are timeouts set?
- Is monitoring in place?
- Is logging appropriate?

**Commands**:
```bash
# Manual reliability review
# Chaos engineering (if applicable)
```

### Lens 6: Supply Chain

**Focus**: Dependencies and licensing

**Questions**:
- Are dependencies up to date?
- Are there known vulnerabilities?
- Are licenses compatible?
- Are there unmaintained dependencies?
- Are there unnecessary dependencies?
- Is provenance verified?

**Commands**:
```bash
cargo outdated
cargo audit
cargo license
# cargo-supply-chain (if available)
```

## Review Process

### Phase 1: Discovery

Identify files to review:
```bash
# Get changed files since base
git diff --name-only origin/main...HEAD

# Get new files
git log --name-only --diff-filter=A origin/main..HEAD
```

### Phase 2: Automated Analysis

Run automated tools:
```bash
# All automated checks
cargo test --all
cargo clippy --all-targets
cargo audit
cargo fmt -- --check
cargo doc --no-deps
```

### Phase 3: Manual Review

Apply each lens:
```bash
# For each lens:
# 1. Read relevant code
# 2. Apply lens questions
# 3. Document findings
# 4. Assign severity
```

### Phase 4: Synthesis

Aggregate findings:
```bash
# Aggregate into findings.log
cat <<EOF > findings.log
[P0] security: src/auth.rs:45 - SQL injection in user lookup
[P1] correctness: src/api.rs:123 - Missing error handling
[P2] maintainability: src/utils.rs:67 - Function name unclear
EOF
```

### Phase 5: Resolution

Handle findings:
```bash
# P0/P1: Must fix
# P2: Document and decide
# Accepted risk: Document
```

## Finding Format

```markdown
[P0] path/to/file:line - Issue description
- Severity: P0 (blocks ship)
- Impact: What happens if this is exploited?
- Evidence: How to reproduce/verify
- Fix: Suggested remediation

[P1] path/to/file:line - Issue description
- Severity: P1 (should fix before ship)
- Impact: Potential impact
- Evidence: How to verify
- Fix: Suggested remediation

[P2] path/to/file:line - Issue description
- Severity: P2 (nice to fix)
- Impact: Low impact
- Evidence: Optional
- Fix: Optional
```

## Review Checklist

```
Correctness:
□ Logic reviewed
□ Edge cases checked
□ Error handling verified
□ Types validated

Security:
□ Input validation checked
□ Auth/Authz verified
□ Secrets protected
□ Audit run

Performance:
□ Algorithm complexity reviewed
□ Memory usage checked
□ Resource cleanup verified
□ Caching appropriate

Maintainability:
□ Code readable
□ Modules separated
□ Tests adequate
□ Documentation current

Reliability:
□ Failures handled
□ Retries appropriate
□ Monitoring in place
□ Logging adequate

Supply Chain:
□ Dependencies updated
□ Audit clean
□ Licenses compatible
□ No unnecessary deps
```

## RFV Loop Integration

Run 3-round RFV with lens distribution:

**Round 1**:
- Correctness
- Security

**Round 2**:
- Performance
- Supply Chain

**Round 3**:
- Maintainability
- Reliability
- Final synthesis

```bash
printf '%s\n' '{"id":1,"op":"framework_rfv_loop","payload":{"operation":"append_round","repo_root":"<path>","round":1,"review_summary":"Correctness: 0 P0, 1 P1, 3 P2. Security: 0 P0, 0 P1, 2 P2","fix_summary":"Fixed P1 in src/api.rs:123","verify_result":"PASS","supervisor_decision":"continue"}}' | router-rs --stdio-json
```
