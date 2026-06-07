---
name: NexBox Glass
colors:
  bg: "#f5f7fb"
  bg-warm: "#eef1f8"
  fg: "#16163a"
  fg-muted: "#5a5a7a"
  accent: "#5865f2"
  accent-secondary: "#7c3aed"
  accent-gradient: "linear-gradient(135deg, #5865f2, #7c3aed)"
  glass: "rgba(255,255,255,0.45)"
  glass-border: "rgba(255,255,255,0.65)"
  glass-shadow: "rgba(88,101,242,0.08)"
  success: "#22c55e"
  warning: "#f59e0b"
typography:
  headline:
    fontFamily: "Space Grotesk"
    fontSize: 5rem
    fontWeight: 900
    letterSpacing: -0.02em
  subhead:
    fontFamily: "Space Grotesk"
    fontSize: 2.5rem
    fontWeight: 700
  body:
    fontFamily: "DM Sans"
    fontSize: 1.25rem
    fontWeight: 300
    lineHeight: 1.6
  label:
    fontFamily: "DM Sans"
    fontSize: 0.875rem
    fontWeight: 500
    textTransform: uppercase
    letterSpacing: 0.1em
  data:
    fontFamily: "JetBrains Mono"
    fontSize: 1.5rem
    fontWeight: 400
    fontVariantNumeric: tabular-nums
rounded:
  sm: 12px
  md: 20px
  lg: 28px
  full: 9999px
spacing:
  sm: 8px
  md: 16px
  lg: 32px
  xl: 64px
motion:
  energy: moderate
  easing:
    entry: "power3.out"
    exit: "power2.in"
    ambient: "sine.inOut"
  duration:
    entrance: 0.5
    hold: 2.0
    transition: 0.6
  atmosphere:
    - floating-glass-shapes
    - radial-accent-glow
    - subtle-grid-pattern
  transition: blur-crossfade
---

## Overview

NexBox is a gaming desktop toolbox. The video showcases its features in a clean, modern white-themed design with liquid glassmorphism effects. Every card uses frosted glass (backdrop-filter blur + semi-transparent white + subtle border). The accent blue-purple gradient provides energy without overwhelming the clean aesthetic.

## Colors

- **Background**: Light cool white (#f5f7fb) — not pure white, tinted slightly blue
- **Text**: Deep navy (#16163a) for headlines, muted (#5a5a7a) for body
- **Accent**: Indigo blue (#5865f2) with purple secondary (#7c3aed) — gaming-oriented
- **Glass**: rgba(255,255,255,0.45) with 24px backdrop blur and white border
- **Success green** (#22c55e) for positive metrics, **warning amber** (#f59e0b) for attention

## Typography

- **Headlines**: Space Grotesk 900 — geometric, bold, modern tech feel
- **Body**: DM Sans 300 — clean, readable, pairs well with Space Grotesk
- **Data/Stats**: JetBrains Mono — monospace for hardware stats and numbers
- **Chinese text**: Falls back to system sans-serif for CJK characters

## Elevation

All cards use glassmorphism:
- `backdrop-filter: blur(24px)`
- `background: rgba(255,255,255,0.45)`
- `border: 1px solid rgba(255,255,255,0.65)`
- `box-shadow: 0 8px 32px rgba(88,101,242,0.08)`

## Do's and Don'ts

- DO use glassmorphism on every card and content panel
- DO keep backgrounds light and airy
- DO use gradient accents sparingly (borders, icons, highlights)
- DON'T use pure #000 or #fff — always tint toward accent
- DON'T stack too many overlapping glass layers (max 2 depth levels)
- DON'T use dark backgrounds — the video is white-themed throughout
