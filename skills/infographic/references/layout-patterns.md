# Infographic Layout Patterns

Common layout patterns and CSS snippets for building HTML infographics.

## Pattern 1: Vertical Flow

Best for: article summaries, step-by-step guides, timelines.

```css
.infographic {
  width: 900px;
  margin: 0 auto;
  font-family: 'DM Sans', sans-serif;
  color: #1E293B;
}

.section {
  padding: 48px 64px;
  position: relative;
}

.section:nth-child(odd) {
  background: #F8FAFC;
}

.section:nth-child(even) {
  background: #FFFFFF;
}

.section-title {
  font-family: 'Space Grotesk', sans-serif;
  font-size: 28px;
  font-weight: 700;
  margin-bottom: 24px;
  color: #0F172A;
}
```

## Pattern 2: Card Grid

Best for: multi-topic overviews, feature comparisons, category breakdowns.

```css
.card-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 24px;
  padding: 48px 64px;
}

.card {
  background: #FFFFFF;
  border-radius: 16px;
  padding: 32px;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  border: 1px solid #E2E8F0;
}

.card-icon {
  font-size: 36px;
  margin-bottom: 16px;
}

.card-title {
  font-size: 20px;
  font-weight: 600;
  margin-bottom: 12px;
}

.card-body {
  font-size: 15px;
  line-height: 1.6;
  color: #475569;
}
```

## Pattern 3: KPI / Metric Strip

Best for: data highlights, statistical summaries, key findings.

```css
.metric-strip {
  display: flex;
  justify-content: space-around;
  padding: 48px 32px;
  background: linear-gradient(135deg, #0F172A, #1E293B);
  color: #FFFFFF;
}

.metric {
  text-align: center;
}

.metric-value {
  font-size: 56px;
  font-weight: 800;
  background: linear-gradient(135deg, #38BDF8, #818CF8);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.metric-label {
  font-size: 14px;
  text-transform: uppercase;
  letter-spacing: 1.5px;
  margin-top: 8px;
  opacity: 0.7;
}
```

## Pattern 4: Timeline

Best for: chronological events, process steps, milestones.

```css
.timeline {
  position: relative;
  padding: 48px 64px;
}

.timeline::before {
  content: '';
  position: absolute;
  left: 120px;
  top: 48px;
  bottom: 48px;
  width: 3px;
  background: linear-gradient(to bottom, #38BDF8, #818CF8);
}

.timeline-item {
  display: flex;
  gap: 32px;
  margin-bottom: 32px;
  align-items: flex-start;
}

.timeline-date {
  width: 80px;
  font-size: 14px;
  font-weight: 600;
  color: #64748B;
  text-align: right;
  flex-shrink: 0;
}

.timeline-dot {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #38BDF8;
  border: 3px solid #FFFFFF;
  box-shadow: 0 0 0 3px #38BDF8;
  flex-shrink: 0;
  margin-top: 4px;
}

.timeline-content {
  flex: 1;
}
```

## Pattern 5: Comparison / Split Panel

Best for: before/after, pros/cons, two-perspective comparisons.

```css
.comparison {
  display: grid;
  grid-template-columns: 1fr 1fr;
}

.comparison-left {
  background: #FEF2F2;
  padding: 48px;
}

.comparison-right {
  background: #F0FDF4;
  padding: 48px;
}

.comparison-header {
  font-size: 24px;
  font-weight: 700;
  margin-bottom: 24px;
}

.comparison-left .comparison-header { color: #DC2626; }
.comparison-right .comparison-header { color: #16A34A; }

.comparison-item {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-bottom: 16px;
  font-size: 15px;
}
```

## Header / Title Section Template

```css
.infographic-header {
  background: linear-gradient(135deg, #0F172A 0%, #1E293B 100%);
  color: #FFFFFF;
  padding: 80px 64px;
  text-align: center;
}

.infographic-header h1 {
  font-family: 'Space Grotesk', sans-serif;
  font-size: 48px;
  font-weight: 800;
  margin-bottom: 16px;
  line-height: 1.2;
}

.infographic-header .subtitle {
  font-size: 18px;
  opacity: 0.8;
  max-width: 600px;
  margin: 0 auto;
  line-height: 1.6;
}
```

## Footer / Source Section Template

```css
.infographic-footer {
  background: #0F172A;
  color: #94A3B8;
  padding: 32px 64px;
  font-size: 12px;
  line-height: 1.8;
}

.infographic-footer .source-label {
  text-transform: uppercase;
  letter-spacing: 1px;
  font-weight: 600;
  margin-bottom: 8px;
  color: #64748B;
}
```

## Google Fonts Import

```css
@import url('https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;600;700&family=DM+Sans:wght@400;500;600;700&display=swap');
```

## Full Template Skeleton

```html
<!DOCTYPE html>
<html lang="zh">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Infographic Title</title>
  <style>
    @import url('https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;600;700;800&family=DM+Sans:wght@400;500;600;700&display=swap');

    * { margin: 0; padding: 0; box-sizing: border-box; }

    .infographic {
      width: 900px;
      margin: 0 auto;
      font-family: 'DM Sans', sans-serif;
      color: #1E293B;
      background: #FFFFFF;
    }

    /* Paste header, sections, footer styles here */
  </style>
</head>
<body>
  <div class="infographic">
    <header class="infographic-header">
      <h1>Title</h1>
      <p class="subtitle">Subtitle or description</p>
    </header>

    <!-- Content sections go here -->

    <footer class="infographic-footer">
      <div class="source-label">Sources</div>
      <p>Data source attribution</p>
    </footer>
  </div>
</body>
</html>
```
