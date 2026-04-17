# Notation Conventions Reference

Reference standards for conducting notation audits. Use as a lookup during
symbol, formula, and unit checks.

## Symbol Font Conventions

Standard academic typesetting conventions. Flag deviations unless the paper
adopts an explicitly different convention and applies it consistently.

| Object Type | Convention | LaTeX Example |
|---|---|---|
| Scalar (real) | Lowercase italic | `$x$`, `$\alpha$` |
| Vector | Lowercase bold italic | `$\mathbf{x}$`, `$\boldsymbol{\mu}$` |
| Matrix | Uppercase bold | `$\mathbf{A}$`, `$\mathbf{W}$` |
| Tensor (order ≥ 3) | Uppercase calligraphic or bold | `$\mathcal{T}$`, `$\boldsymbol{\mathcal{T}}$` |
| Set | Uppercase calligraphic | `$\mathcal{S}$`, `$\mathcal{D}$` |
| Number set | Blackboard bold | `$\mathbb{R}$`, `$\mathbb{Z}$` |
| Random variable | Uppercase italic | `$X$`, `$Y$` |
| Function / operator | Roman (upright) or named | `$\operatorname{ReLU}$`, `$\log$`, `$\max$` |
| Constant | Roman (upright) | `$\mathrm{e}$` (Euler), `$\mathrm{i}$` (imaginary) |
| Unit | Roman (upright) | `$\mathrm{m}$`, `$\mathrm{kg}$`, `$\mathrm{s}$` |

## Abbreviation Standards

### Commonly accepted no-expansion abbreviations

These are widely understood in CS/ML/AI papers and typically do not require
expansion. However, if the venue explicitly requires all abbreviations to be
expanded, expand them anyway.

| Abbreviation | Full Form |
|---|---|
| AI | Artificial Intelligence |
| ML | Machine Learning |
| DL | Deep Learning |
| CNN | Convolutional Neural Network |
| RNN | Recurrent Neural Network |
| GAN | Generative Adversarial Network |
| NLP | Natural Language Processing |
| GPU | Graphics Processing Unit |
| CPU | Central Processing Unit |
| API | Application Programming Interface |
| IoT | Internet of Things |
| LSTM | Long Short-Term Memory |
| MLP | Multilayer Perceptron |
| SGD | Stochastic Gradient Descent |
| SVM | Support Vector Machine |

### Mandatory expansion abbreviations

Any abbreviation **not** in the list above, or any domain-specific acronym,
**must** be expanded at first use. Examples:

- PINN → Physics-Informed Neural Network
- FNO → Fourier Neural Operator
- ViT → Vision Transformer
- RLHF → Reinforcement Learning from Human Feedback

### Multilingual rules (Chinese papers)

- English abbreviations in Chinese text still require full **English** expansion
  at first use, e.g., "卷积神经网络 (Convolutional Neural Network, CNN)"
- If the Chinese full name is also given, put it before the English expansion
- Do not abbreviate Chinese terms unless the venue explicitly allows it

## Formula Punctuation Rules

Displayed equations are part of sentences. Apply standard punctuation:

| Scenario | Rule | Example |
|---|---|---|
| Equation ends a sentence | Period after equation | `$$E = mc^2.$$ ` |
| Equation is followed by more text | Comma after equation | `$$f(x) = ax + b,$$ where $a$ is the slope.` |
| Equation is followed by "where" | Comma after equation | `$$L = L_{\text{data}} + \lambda L_{\text{reg}},$$ where $\lambda$ controls…` |
| Multiple aligned equations | Comma after each, period after last | See `align` environment |
| Equation followed by "and" or continuation | No punctuation or comma, depending on grammar | Context-dependent |

### "Where" block format

After a displayed equation, the "where" block should:
1. Start on a new line (or same line if short)
2. Use consistent formatting: `where $x$ is …, $y$ is …, and $z$ is …`
3. Define every new symbol introduced in that equation
4. Not re-define symbols already defined earlier (unless redefining)

## Unit Formatting Rules

### SI conventions

| Rule | Correct | Incorrect |
|---|---|---|
| Upright font for units | $10\,\mathrm{kg}$ | $10\,kg$ (italic) |
| Space between number and unit | $10\,\mathrm{m}$ | $10\mathrm{m}$ (no space) |
| Multiplication in compound units | $\mathrm{N \cdot m}$ or $\mathrm{N\,m}$ | $\mathrm{Nm}$ (ambiguous) |
| Division | $\mathrm{m/s}$ or $\mathrm{m\,s^{-1}}$ | $\mathrm{m/s/s}$ (nested slash) |
| Percent | $10\,\%$ with space | $10\%$ (acceptable in some styles) |

### LaTeX packages

Recommend `siunitx` for consistent unit formatting:
```latex
\usepackage{siunitx}
\SI{10}{\kilo\gram}     % → 10 kg
\SI{3.0e8}{\meter\per\second}  % → 3.0 × 10⁸ m/s
\si{\milli\second}       % → ms (unit only)
```

### Common confusions

| Intended | Correct | Common Error |
|---|---|---|
| Millisecond | ms | mS (milliSiemens) |
| Microsecond | µs | us |
| Kilowatt-hour | kW·h | KWh, kwh |
| Decibel | dB | db, DB |
