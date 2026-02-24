# Quedo — Branding & Identity

## Name
**Quedo** (from Spanish "quedo" — soft, gentle, hushed)

Tagline ideas: "Voice, quietly typed." / "Speak softly, type everything."

## App Icon
Source file: `macos/Assets/quedo-app-icon.png`

Design: Small dot (sound source) with two warm cream/champagne arcs radiating outward, on matte black background. Minimal, understated, premium Mac aesthetic.

Colors:
- Background: pure matte black (#000000 or near-black)
- Arcs/dot: warm cream/champagne (#E8DCC8 approximate)
- No gradients in the mark itself, subtle ambient glow optional

## Asset Catalog Setup (for Mac agent)
The app icon needs to be set up as an Xcode asset catalog:

1. Create `macos/Sources/Quedo/Assets.xcassets/AppIcon.appiconset/`
2. Generate required sizes from `quedo-app-icon.png`:
   - 16x16, 16x16@2x (32px)
   - 32x32, 32x32@2x (64px)
   - 128x128, 128x128@2x (256px)
   - 256x256, 256x256@2x (512px)
   - 512x512, 512x512@2x (1024px)
3. Create `Contents.json` with the size mappings
4. Reference in the app target

## Menu Bar Icon
Derive a simplified version for NSStatusItem (menu bar):
- Monochrome (template image) — just the two arcs + dot in white
- 18x18pt (36px @2x) for menu bar
- Must work as an SF Symbols-style template image
- States: idle (normal), recording (filled/highlighted), processing (animated pulse)

## Rename Checklist
The codebase currently uses "WhisperAssistant" everywhere. Rename to "Quedo":
- [x] SPM package name and targets in `Package.swift`
- [x] All `WhisperAssistantCore` → `QuedoCore`
- [x] All `WhisperAssistant` (app target) → `Quedo`
- [x] All `WhisperAssistantCLI` → `QuedoCLI`
- [x] Directory names under `Sources/` and `Tests/`
- [x] Bundle identifier: `com.futhark.quedo` (or similar)
- [x] README, HANDOFF.md references
- [x] GitHub release workflow
- [x] CLI binary name: `quedo` (not `whisper-assistant`)

## Domain
`quedo.app` — likely available (not registered as of 2026-02-24)
