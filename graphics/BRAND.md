# StateSync brand kit (agent-facing)

Agents must follow this file (and org kit if present) when changing icons, headers, or README art.

## Org icon style (studio2201) — do not reinvent

Canonical app icons across the org share this look:

| Rule | Spec |
|------|------|
| Shape | **Square** mark; often soft **rounded square** app-icon framing |
| Size | Prefer **1024×1024** master; export **512** for README; keep a JPEG or PNG for Unraid |
| Background | Flat **deep navy / charcoal** (`#0a1628` … `#1a1a2e` / `#0b0f14`) — not busy photos |
| Foreground | **Single symbolic glyph** for the product (waveform, sync arrows, share, etc.) |
| Line style | **Neon line art / geometric UI icon**, even stroke weight, high contrast |
| Colors | Limited palette: **cyan** (`#00e5ff` / `#3b9eff`) + **green or purple** accent; 2–3 colors max |
| Text | **None.** No letters, numbers, logos, or watermarks inside the image |
| Depth | Flat or soft glow only — not 3D clay, not photoreal |
| Consistency | Match sibling repos: `pulse/assets/icon.png`, `beam/assets/icon.png`, `pad/assets/icon.png` |

### StateSync icon (locked)

- **Canonical:** `graphics/statesync_icon.jpg` (cyan + green dual circular arrows)
- **README export:** `assets/icon.png` (must be a resize of the same art only)
- **Unraid Icon URL:** `…/graphics/statesync_icon.jpg`
- **Never replace** the app icon with character art, mascots, or headers

### How to keep agents on-style

Put durable rules in one or more of:

1. **`graphics/BRAND.md`** (this file) — per-repo visual law  
2. **`AGENT.md`** — short pointer: “icons follow `graphics/BRAND.md`; never swap the glyph icon”  
3. **Org-level** `studio2201/.github/` or `shared-assets/BRAND.md` — palette + icon rules for all apps  
4. **Reference paths** listed in AGENT.md:  
   `../pulse/assets/icon.png`, `../beam/assets/icon.png` as visual ground truth  
5. **When generating:** say “match studio2201 neon line-icon style; square; no text; do not change product icon unless asked”

## Header banners (separate from icons)

Headers may use character / scene art. Icons may not.

| Rule | Spec |
|------|------|
| Aspect | Wide **~16:9** for README |
| Text | **None** |
| Palette | Same dark ground + cyan/green accents so it sits with org dark UI |
| Role | README top banner only — not the app icon, not Unraid tray icon |

### StateSync header (current)

- **Active:** `assets/header.png` (2D cell-animation style; pleated skirt; not wartime)  
- **Source archive:** `graphics/statesync_header_cell.jpg` (or latest pinup/cell masters)  
- **Neon geometric archive:** `graphics/statesync_header_neon.jpg`  
- Optional mascot stills stay under `assets/mascot.png` / `graphics/statesync_mascot.png` and must **not** replace the icon

## README layout (org)

```html
<p align="center"><img src="assets/header.png" …></p>
# <img src="assets/icon.png" width="32" …> ProductName
```

No ASCII art, no emoji in headings. Blue Ocean: one-line install first.
