# Mermaid Expert Implementation Playbook

Use this playbook when the user needs concrete Mermaid patterns instead of only
high-level advice.

## 1. Research flowchart template

```mermaid
flowchart TD
    A[Research Question] --> B[Data Collection]
    B --> C[Preprocessing]
    C --> D[Method Stage 1]
    D --> E{Quality Check}
    E -- Pass --> F[Method Stage 2]
    E -- Fail --> C
    F --> G[Evaluation]
    G --> H[Results]
```

Best for:
- 研究流程图
- 实验流程
- 方法总览图

## 1b. Literature screening / PRISMA-like template

```mermaid
flowchart TD
    A[Records Identified\nn = 1200] --> B[Duplicates Removed\nn = 180]
    B --> C[Records Screened\nn = 1020]
    C --> D{Meets Title/Abstract Criteria?}
    D -- Yes --> E[Full-text Review\nn = 210]
    D -- No --> F[Excluded Early\nn = 810]
    E --> G{Eligible After Full-text Review?}
    G -- Yes --> H[Studies Included\nn = 58]
    G -- No --> I[Excluded with Reasons\nn = 152]
```

Best for:
- 文献筛选流程图
- PRISMA-like 草图
- 纳入/排除流程

## 2. Technical roadmap template

```mermaid
gantt
    title Research Roadmap
    dateFormat  YYYY-MM-DD

    section Phase 1
    Literature Review     :a1, 2026-03-01, 21d
    Problem Formulation   :a2, after a1, 14d

    section Phase 2
    Prototype             :b1, after a2, 21d
    Initial Experiments   :b2, after b1, 21d

    section Phase 3
    Evaluation            :c1, after b2, 21d
    Paper Writing         :c2, after c1, 14d
```

Best for:
- 技术路线图
- 研究计划
- 论文时间线

## 3. Method pipeline with grouped modules

```mermaid
flowchart LR
    A[Input Data] --> B

    subgraph P1[Preprocessing]
        B[Cleaning]
        C[Feature Extraction]
        B --> C
    end

    C --> D

    subgraph P2[Core Method]
        D[Encoder]
        E[Fusion Module]
        F[Decoder]
        D --> E --> F
    end

    F --> G[Predictions]
    G --> H[Evaluation Metrics]
```

Best for:
- 方法图
- 模型 pipeline
- 系统处理流程

## 3b. Training / inference split

```mermaid
flowchart LR
    A[Raw Dataset] --> B[Preprocessing]
    B --> C[Train/Val Split]

    subgraph T[Training]
        C --> D[Model Training]
        D --> E[Checkpoint Selection]
    end

    subgraph I[Inference]
        E --> F[Inference on Test Set]
        F --> G[Post-processing]
    end

    G --> H[Metrics and Error Analysis]
```

Best for:
- 训练流程
- 推理流程
- 评测 pipeline

## 4. Sequence diagram template

```mermaid
sequenceDiagram
    autonumber
    participant U as User
    participant C as Client
    participant S as Server
    participant D as Database

    U->>C: Submit request
    C->>S: POST /run
    S->>D: Query records
    D-->>S: Return results
    S-->>C: Response payload
    C-->>U: Render output
```

Best for:
- API 时序图
- agent / service interaction
- 协议流程

## 5. ER diagram template

```mermaid
erDiagram
    PROJECT ||--o{ EXPERIMENT : contains
    EXPERIMENT ||--o{ RUN : produces
    RUN ||--o{ METRIC : records

    PROJECT {
        string id
        string name
    }
    EXPERIMENT {
        string id
        string hypothesis
    }
    RUN {
        string id
        string status
    }
    METRIC {
        string id
        float score
    }
```

Best for:
- 数据模型
- 实验记录结构
- 实体关系梳理

## 6. Architecture diagram template

```mermaid
flowchart LR
    U[Users] --> FE[Frontend]
    FE --> API[API Layer]

    subgraph APP[Application Services]
        S1[Service A]
        S2[Service B]
    end

    API --> S1
    API --> S2
    S1 --> DB[(Database)]
    S2 --> CACHE[(Cache)]
```

## 7. Practical rules

- If the user says “流程” but the real need is interaction timing, switch to `sequenceDiagram`
- If the user says “技术路线图” and includes dates/phases, prefer `gantt`
- If a flowchart grows beyond ~12 primary nodes, propose splitting it
- If labels become sentences, shorten labels and move detail into notes
- If the user wants “论文图” but also wants strict publication polish, warn that Mermaid is best for editable source diagrams, not always final camera-ready art
- If the user wants Word-style strict orthogonal flow layout, suggest Graphviz/DOT rather than stretching Mermaid past its strengths

## 8. Delivery pattern

Recommended response structure:

````markdown
## Mermaid Diagram
```mermaid
...
```

## Why this structure
- ...

## Assumptions
- ...
````

Optional add-on:

```markdown
## Caption
Figure X. Overview of the proposed workflow from data preparation to evaluation.
```
