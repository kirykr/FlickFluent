# Contract: caption templates

`config/caption-templates.json`, an array in the order shown to the user. The core validates it at start and refuses a file with a missing field, a `fontFamily` that names no Khmer fallback, a colour that is not `#rrggbb` or `#rrggbbaa`, or a size outside 2 to 12 percent.

```json
[
  {
    "id": "classic",
    "nameKey": "editor.captions.classic",
    "fontFamily": "Inter, \"Noto Sans Khmer\", \"Khmer UI\", \"Khmer Sangam MN\", sans-serif",
    "sizePercent": 5,
    "color": "#ffffff",
    "outlineColor": "#000000",
    "outlineWidth": 2,
    "background": null,
    "position": "bottom",
    "marginPercent": 6
  }
]
```

Three ship: `classic`, `soft-box` (dark box `#000000b3`, no outline), `top-note` (yellow `#ffd84a`, top). The name keys exist in every language file. Adding a template is a data change only.

**Preview**: drawn over the video frame in the UI from these fields.
**Export**: WebVTT `STYLE` and cue settings from these fields (see `subtitle-formats.md`). SRT ignores the template and the export dialog says so.
