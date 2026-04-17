---
name: youtube-summarizer
description: |
  Extract transcripts from YouTube videos and turn them into summaries, notes,
  key takeaways, timestamps, and structured content analysis. Use when the user
  provides a YouTube URL and wants 总结视频、提取字幕、整理笔记、输出重点, or a
  detailed breakdown without rewatching the video; not for non-YouTube video
  platforms unless the workflow is explicitly adapted.
routing_layer: L4
routing_owner: owner
routing_gate: none
session_start: n/a
metadata:
  version: "1.2.1"
  platforms: [codex]
  tags:
    - youtube
    - summarization
    - transcription
risk: safe
source: community
---
# youtube-summarizer

## Purpose

Extracts transcripts from YouTube videos and generates comprehensive summaries using the STAR + R-I-S-E framework. Validates video availability, extracts transcripts using `youtube-transcript-api`, and produces detailed documentation capturing all insights, arguments, and key points.

## When to use

- The user provides a YouTube video URL and wants a detailed summary
- The user needs to document video content for reference without rewatching
- The user wants to extract insights and key points from educational content
- The user asks to "summarize", "resume", or "extract content" from YouTube videos
- The user says "总结视频", "提取字幕", "整理视频要点", "YouTube transcript"
- The task involves turning video content into structured notes, key takeaways, or timestamps

## Do not use

- The task is about general web content summarization (not YouTube) → just read the URL directly
- The task is about creating video content rather than summarizing existing videos
- The task is about podcast or audio transcription without a YouTube source

## Step 0: Setup

```bash
# Check if youtube-transcript-api is installed
python3 -c "import youtube_transcript_api" 2>/dev/null || pip install youtube-transcript-api
```

## Main Workflow

### Step 1: Validate YouTube URL

Supported formats:
- `https://www.youtube.com/watch?v=VIDEO_ID`
- `https://youtu.be/VIDEO_ID`
- `https://m.youtube.com/watch?v=VIDEO_ID`

```bash
# Extract video ID
URL="$USER_PROVIDED_URL"
if echo "$URL" | grep -qE 'youtube\.com/watch\?v='; then
    VIDEO_ID=$(echo "$URL" | sed -E 's/.*[?&]v=([^&]+).*/\1/')
elif echo "$URL" | grep -qE 'youtu\.be/'; then
    VIDEO_ID=$(echo "$URL" | sed -E 's/.*youtu\.be\/([^?]+).*/\1/')
fi
```

### Step 2: Extract Transcript

```python
from youtube_transcript_api import YouTubeTranscriptApi

transcript = YouTubeTranscriptApi.get_transcript(
    video_id,
    languages=['zh', 'en']  # Prefer Chinese, fallback to English
)
full_text = " ".join([entry['text'] for entry in transcript])
```

**Error Handling:**

| Error | Action |
|-------|--------|
| Transcripts disabled | Cannot proceed, inform user |
| No transcript found | Cannot proceed, inform user |
| Private/restricted video | Ask for public video |

### Step 3: Generate Comprehensive Summary

Apply STAR + R-I-S-E framework:
1. Load the full transcript text
2. Generate structured summary with headers
3. Organize by topic sections

### Step 4: Output Format

```markdown
# [Video Title]

**URL:** [YouTube URL]

## 📝 Executive Summary
[2-3 paragraph overview]

## 📖 Detailed Breakdown

### [Topic 1]
[Comprehensive explanation with examples...]

### [Topic 2]
[Continued analysis...]

## 💡 Key Insights
- [Insight 1]
- [Insight 2]

## 📚 Concepts and Terminology
- **[Term]:** [Definition and context]

## 📌 Conclusion
[Final synthesis and takeaways]
```

## Progress Display

```
╔══════════════════════════════════════════════════════╗
║     📹  YOUTUBE SUMMARIZER - Processing Video        ║
╠══════════════════════════════════════════════════════╣
║ → Step 1: Validating URL              [IN PROGRESS]  ║
║ ○ Step 2: Extracting Transcript                      ║
║ ○ Step 3: Generating Summary                         ║
╠══════════════════════════════════════════════════════╣
║ Progress: ██████░░░░░░░░░░░░░░  33%                  ║
╚══════════════════════════════════════════════════════╝
```
